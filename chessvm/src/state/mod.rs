//! Manages the virtual machine states.

use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    io::{self, Error, ErrorKind},
    sync::Arc,
};

use crate::block::Block;
use avalanche_types::{choices, ids, subnet};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use alloy_primitives::Address;
use shakmaty::{Chess, Color, Move, Position};

#[derive(Clone)]
pub struct GameState {
    game: Chess,
    white: Address,
    black: Address,
}

/// Manages block and chain states for this Vm, both in-memory and persistent.
#[derive(Clone)]
pub struct State {
    pub db: Arc<RwLock<Box<dyn subnet::rpc::database::Database + Send + Sync>>>,

    /// Maps block Id to Block.
    /// Each element is verified but not yet accepted/rejected (e.g., preferred).
    pub verified_blocks: Arc<RwLock<HashMap<ids::Id, Block>>>,

    pub game_states: Arc<RwLock<HashMap<u64, GameState>>>,
}

impl Default for State {
    fn default() -> State {
        Self {
            db: Arc::new(RwLock::new(subnet::rpc::database::memdb::Database::new())),
            verified_blocks: Arc::new(RwLock::new(HashMap::new())),
            game_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

const LAST_ACCEPTED_BLOCK_KEY: &[u8] = b"last_accepted_block";

const STATUS_PREFIX: u8 = 0x0;

const DELIMITER: u8 = b'/';

/// Returns a vec of bytes used as a key for identifying blocks in state.
/// '`STATUS_PREFIX`' + '`BYTE_DELIMITER`' + [`block_id`]
fn block_with_status_key(blk_id: &ids::Id) -> Vec<u8> {
    let mut k: Vec<u8> = Vec::with_capacity(ids::LEN + 2);
    k.push(STATUS_PREFIX);
    k.push(DELIMITER);
    k.extend_from_slice(&blk_id.to_vec());
    k
}

pub fn calculate_game_id(white: Address, black: Address) -> u64 {
    let mut combined_addresses = Vec::new();
    combined_addresses.extend_from_slice(white.as_slice());
    combined_addresses.extend_from_slice(black.as_slice());

    let mut hasher = DefaultHasher::new();
    combined_addresses.hash(&mut hasher);
    hasher.finish()
}

/// Wraps a [`Block`](crate::block::Block) and its status.
/// This is the data format that [`State`](State) uses to persist blocks.
#[derive(Serialize, Deserialize, Clone)]
struct BlockWithStatus {
    block_bytes: Vec<u8>,
    status: choices::status::Status,
}

impl BlockWithStatus {
    fn encode(&self) -> io::Result<Vec<u8>> {
        serde_json::to_vec(&self).map_err(|e| {
            Error::new(
                ErrorKind::Other,
                format!("failed to serialize BlockStatus to JSON bytes: {e}"),
            )
        })
    }

    fn from_slice(d: impl AsRef<[u8]>) -> io::Result<Self> {
        let dd = d.as_ref();
        serde_json::from_slice(dd).map_err(|e| {
            Error::new(
                ErrorKind::Other,
                format!("failed to deserialize BlockStatus from JSON: {e}"),
            )
        })
    }
}

impl State {
    /// Persists the last accepted block Id to state.
    /// # Errors
    /// Fails if the db can't be updated
    pub async fn set_last_accepted_block(&self, blk_id: &ids::Id) -> io::Result<()> {
        let mut db = self.db.write().await;
        db.put(LAST_ACCEPTED_BLOCK_KEY, &blk_id.to_vec())
            .await
            .map_err(|e| {
                Error::new(
                    ErrorKind::Other,
                    format!("failed to put last accepted block: {e:?}"),
                )
            })
    }

    /// Returns "true" if there's a last accepted block found.
    /// # Errors
    /// Fails if the db can't be read
    pub async fn has_last_accepted_block(&self) -> io::Result<bool> {
        let db = self.db.read().await;
        match db.has(LAST_ACCEPTED_BLOCK_KEY).await {
            Ok(found) => Ok(found),
            Err(e) => Err(Error::new(
                ErrorKind::Other,
                format!("failed to load last accepted block: {e}"),
            )),
        }
    }

    /// Returns the last accepted block Id from state.
    /// # Errors
    /// Can fail if the db can't be read
    pub async fn get_last_accepted_block_id(&self) -> io::Result<ids::Id> {
        let db = self.db.read().await;
        match db.get(LAST_ACCEPTED_BLOCK_KEY).await {
            Ok(d) => Ok(ids::Id::from_slice(&d)),
            Err(e) => {
                if subnet::rpc::errors::is_not_found(&e) {
                    return Ok(ids::Id::empty());
                }
                Err(e)
            }
        }
    }

    /// Adds a block to "`verified_blocks`".
    pub async fn add_verified(&mut self, block: &Block) {
        let blk_id = block.id();
        // log::info!(
        //     "verified added {blk_id} with {} num of TXs",
        //     block.get_num_of_transactions()
        // );

        let mut verified_blocks = self.verified_blocks.write().await;
        verified_blocks.insert(blk_id, block.clone());
    }

    /// Removes a block from "`verified_blocks`".
    pub async fn remove_verified(&mut self, blk_id: &ids::Id) {
        let mut verified_blocks = self.verified_blocks.write().await;
        verified_blocks.remove(blk_id);
    }

    /// Returns "true" if the block Id has been already verified.
    pub async fn has_verified(&self, blk_id: &ids::Id) -> bool {
        let verified_blocks = self.verified_blocks.read().await;
        verified_blocks.contains_key(blk_id)
    }

    /// Writes a block to the state storage.
    /// # Errors
    /// Can fail if the block fails to serialize or if the db can't be updated
    pub async fn write_block(&mut self, block: &Block) -> io::Result<()> {
        let blk_id = block.id();
        let blk_bytes = block.to_vec()?;

        let mut db = self.db.write().await;

        let blk_status = BlockWithStatus {
            block_bytes: blk_bytes,
            status: block.status(),
        };
        let blk_status_bytes = blk_status.encode()?;

        db.put(&block_with_status_key(&blk_id), &blk_status_bytes)
            .await
            .map_err(|e| Error::new(ErrorKind::Other, format!("failed to put block: {e:?}")))
    }

    /// Reads a block from the state storage using the `block_with_status_key`.
    /// # Errors
    /// Can fail if the block is not found in the state storage, or if the block fails to deserialize
    pub async fn get_block(&self, blk_id: &ids::Id) -> io::Result<Block> {
        // check if the block exists in memory as previously verified.
        let verified_blocks = self.verified_blocks.read().await;
        if let Some(b) = verified_blocks.get(blk_id) {
            return Ok(b.clone());
        }

        let db = self.db.read().await;

        let blk_status_bytes = db.get(&block_with_status_key(blk_id)).await?;
        let blk_status = BlockWithStatus::from_slice(blk_status_bytes)?;

        let mut blk = Block::from_slice(&blk_status.block_bytes)?;
        blk.set_status(blk_status.status);

        Ok(blk)
    }

    /// Creates a new chess game without making a move
    pub async fn create_new_game(&self, white: Address, black: Address) -> io::Result<u64> {
        let mut game_states = self.game_states.write().await;

        let new_game = Chess::default();
        let new_game_state = GameState {
            game: new_game,
            white,
            black,
        };

        // Need to create game ID
        let game_id = calculate_game_id(white, black);

        game_states.insert(game_id, new_game_state);

        Ok(game_id)
    }

    /// Makes a move on an already existing chess board
    pub async fn make_move(&self, player: Address, game_id: u64, mv: &Move) -> io::Result<()> {
        // Retrieve game board from state
        let mut game_states = self.game_states.write().await;

        if let None = game_states.get(&game_id) {
            return Err(Error::new(
                ErrorKind::Other,
                format!("Game does not exist!"),
            ));
        }

        // Game exists, we can unwrap directly without panicking
        let mut curr_game = game_states.get(&game_id).unwrap().clone();

        // Check if player can make move
        if curr_game.game.turn() == Color::White {
            if player != curr_game.white {
                return Err(Error::new(ErrorKind::Other, "It is not the player's turn!"));
            }
        } else {
            if player != curr_game.black {
                return Err(Error::new(ErrorKind::Other, "It is not the player's turn!"));
            }
        }

        if !curr_game.game.is_legal(mv) {
            return Ok(());
        }

        // Player can make the move, we update the game state and write back
        if let Ok(v) = curr_game.game.play(mv) {
            // Update game state
            curr_game.game = v;
            // Write back to state
            game_states.insert(game_id, curr_game);

            return Ok(());
        }

        Err(Error::new(ErrorKind::Other, "MakeMove Failed!"))
    }

    /// Ends a chess game, if possible
    pub async fn end_game(&self, game_id: u64) -> io::Result<Chess> {
        // Get write access to state
        let mut game_states = self.game_states.write().await;

        // If game not found
        if !game_states.contains_key(&game_id) {
            return Err(Error::new(ErrorKind::Other, "Game not found!"));
        }

        // Game exists, we now remove
        Ok(game_states.remove(&game_id).unwrap().game)
    }

    /// Getter for game board
    pub async fn get_game(&self, game_id: u64) -> Option<Chess> {
        // Get read access to state
        let game_states = self.game_states.read().await;

        if let Some(v) = game_states.get(&game_id) {
            return Some(v.clone().game);
        }

        None
    }

    /// Returns `true` if a game exists, `false` otherwise
    pub async fn game_exists(&self, game_id: u64) -> bool {
        let game_states = self.game_states.read().await;

        game_states.contains_key(&game_id)
    }
}
