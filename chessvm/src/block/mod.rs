use alloy_primitives::Address;
use derivative::{self, Derivative};
use serde::{Deserialize, Serialize};

use std::{
    fmt,
    io::{self, Error, ErrorKind},
};

use avalanche_types::{
    choices::{self, status::Status},
    ids,
    subnet::rpc::consensus::snowman::{self, Decidable},
};
use chrono::{Duration, Utc};
use serde_with::serde_as;

use crate::state;

pub mod tx;

#[serde_as]
#[derive(Serialize, Deserialize, Clone, Derivative, Default)]
#[derivative(Debug, PartialEq, Eq)]
pub struct Block {
    /// The block ID of the parent block
    parent_id: ids::Id,
    /// Height of block
    /// The height of the genesis block is 0
    height: u64,
    /// Unix time of when this block was proposed
    timestamp: u64,
    /// Block Message
    message: String,
    // Transactions
    #[derivative(PartialEq = "ignore")]
    txs: Vec<tx::Transaction>,
    /// Generated block Id.
    #[serde(skip)]
    id: ids::Id,
    /// Current block status.
    #[serde(skip)]
    status: choices::status::Status,

    /// This block's encoded bytes.
    #[serde(skip)]
    bytes: Vec<u8>,

    /// Reference to the VM state manager for blocks
    #[derivative(Debug = "ignore", PartialEq = "ignore")]
    #[serde(skip)]
    state: state::State,
}

impl Block {
    /// Encodes the [`Block`](Block) to JSON in bytes.
    /// # Errors
    /// Errors if the block can't be serialized to JSON.
    pub fn to_vec(&self) -> io::Result<Vec<u8>> {
        serde_json::to_vec(&self).map_err(|e| {
            Error::new(
                ErrorKind::Other,
                format!("failed to serialize Block to JSON bytes {e}"),
            )
        })
    }
    /// Returns the ID of this block
    #[must_use]
    pub fn id(&self) -> ids::Id {
        self.id
    }

    pub fn get_num_of_transactions(&self) -> usize {
        self.txs.len()
    }

    /// Returns the status of this block.
    #[must_use]
    pub fn status(&self) -> choices::status::Status {
        self.status.clone()
    }

    /// Loads [`Block`](Block) from JSON bytes.
    /// # Errors
    /// Will fail if the block can't be deserialized from JSON.
    pub fn from_slice(d: impl AsRef<[u8]>) -> io::Result<Self> {
        let dd = d.as_ref();
        let mut b: Self = serde_json::from_slice(dd).map_err(|e| {
            Error::new(
                ErrorKind::Other,
                format!("failed to deserialize Block from JSON {e}"),
            )
        })?;

        b.bytes = dd.to_vec();
        b.id = ids::Id::sha256(&b.bytes);

        Ok(b)
    }

    /// Updates the status of this block.
    pub fn set_status(&mut self, status: choices::status::Status) {
        self.status = status;
    }

    /// Updates the state of the block.
    pub fn set_state(&mut self, state: state::State) {
        self.state = state;
    }

    /// Verifies [`Block`](Block) properties (e.g., heights),
    /// and once verified, records it to the [`State`](crate::state::State).
    /// # Errors
    /// Can fail if the parent block can't be retrieved.
    pub async fn verify(&mut self) -> io::Result<()> {
        if self.height == 0 && self.parent_id == ids::Id::empty() {
            log::debug!(
                "block {} has an empty parent Id since it's a genesis block -- skipping verify",
                self.id
            );
            self.state.add_verified(&self.clone()).await;
            return Ok(());
        }

        // if already exists in database, it means it's already accepted
        // thus no need to verify once more
        if self.state.get_block(&self.id).await.is_ok() {
            log::debug!("block {} already verified", self.id);
            return Ok(());
        }

        let prnt_blk = self.state.get_block(&self.parent_id).await?;

        // ensure the height of the block is immediately following its parent
        if prnt_blk.height != self.height - 1 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "parent block height {} != current block height {} - 1",
                    prnt_blk.height, self.height
                ),
            ));
        }

        // ensure block timestamp is after its parent
        if prnt_blk.timestamp > self.timestamp {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "parent block timestamp {} > current block timestamp {}",
                    prnt_blk.timestamp, self.timestamp
                ),
            ));
        }

        let one_hour_from_now = Utc::now() + Duration::hours(1);
        let one_hour_from_now = one_hour_from_now
            .timestamp()
            .try_into()
            .expect("failed to convert timestamp from i64 to u64");

        // ensure block timestamp is no more than an hour ahead of this nodes time
        if self.timestamp >= one_hour_from_now {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "block timestamp {} is more than 1 hour ahead of local time",
                    self.timestamp
                ),
            ));
        }

        // add newly verified block to memory
        self.state.add_verified(&self.clone()).await;
        Ok(())
    }

    /// Returns the height of this block.
    #[must_use]
    pub fn height(&self) -> u64 {
        self.height
    }

    pub fn try_new(
        parent_id: ids::Id,
        height: u64,
        timestamp: u64,
        message: String,
        txs: Vec<tx::Transaction>,
        status: choices::status::Status,
    ) -> io::Result<Self> {
        let mut b = Self {
            parent_id,
            height,
            timestamp,
            message,
            txs,
            ..Default::default()
        };
        b.status = status;
        b.bytes = b.to_vec()?;
        b.id = ids::Id::sha256(&b.bytes);

        Ok(b)
    }

    // pub fn insert_tx(&mut self, tx: tx::Transaction) -> io::Result<()> {
    //     self.txs.push(tx);

    //     Ok(())
    // }

    /// Mark this [`Block`](Block) accepted and updates [`State`](crate::state::State) accordingly.
    /// # Errors
    /// Returns an error if the state can't be updated.
    pub async fn accept(&mut self) -> io::Result<()> {
        self.set_status(Status::Accepted);

        // Construct TX context
        // TODO: Fix TX ID
        let tx_context = tx::TransactionContext {
            state: self.state.clone(),
            block_time: self.timestamp,
            tx_id: ids::Id::default(),
            sender: Address::default(),
        };

        // Iterate over each transaction and execute
        for tx in self.txs.iter() {
            tx.execute(tx_context.clone()).await?;
        }

        self.state.write_block(&self.clone()).await?;
        self.state.set_last_accepted_block(&self.id()).await?;

        self.state.remove_verified(&self.id()).await;

        Ok(())
    }

    /// Mark this [`Block`](Block) rejected and updates [`State`](crate::state::State) accordingly.
    /// # Errors
    /// Returns an error if the state can't be updated.
    pub async fn reject(&mut self) -> io::Result<()> {
        self.set_status(Status::Rejected);

        self.state.write_block(&self.clone()).await?;
        self.state.remove_verified(&self.id()).await;
        Ok(())
    }

    /// # Errors
    /// Can fail if the block can't be serialized to JSON.
    pub fn to_json_string(&self) -> io::Result<String> {
        serde_json::to_string(&self).map_err(|e| {
            Error::new(
                ErrorKind::Other,
                format!("failed to serialize Block to JSON string {e}"),
            )
        })
    }
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let serialized = self.to_json_string().unwrap();
        write!(f, "{serialized}")
    }
}

#[tonic::async_trait]
impl snowman::Block for Block {
    async fn bytes(&self) -> &[u8] {
        return self.bytes.as_ref();
    }

    async fn height(&self) -> u64 {
        self.height
    }

    async fn timestamp(&self) -> u64 {
        self.timestamp
    }

    async fn parent(&self) -> ids::Id {
        self.parent_id
    }

    async fn verify(&mut self) -> io::Result<()> {
        self.verify().await
    }
}

#[tonic::async_trait]
impl Decidable for Block {
    /// Implements "snowman.Block.choices.Decidable"
    async fn status(&self) -> choices::status::Status {
        self.status.clone()
    }

    async fn id(&self) -> ids::Id {
        self.id
    }

    async fn accept(&mut self) -> io::Result<()> {
        self.accept().await
    }

    async fn reject(&mut self) -> io::Result<()> {
        self.reject().await
    }
}

#[tokio::test]
async fn test_block() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();

    let mut genesis_blk = Block::try_new(
        ids::Id::empty(),
        0,
        Utc::now().timestamp() as u64,
        String::from("Genesis Block!"),
        Vec::new(),
        choices::status::Status::default(),
    )
    .unwrap();
    log::info!("deserialized: {genesis_blk} (block Id: {})", genesis_blk.id);

    let serialized = genesis_blk.to_vec().unwrap();
    let deserialized = Block::from_slice(&serialized).unwrap();
    log::info!("deserialized: {deserialized}");

    assert_eq!(genesis_blk, deserialized);

    let state = state::State::default();
    assert!(!state.has_last_accepted_block().await.unwrap());

    // inner db instance is protected with arc and mutex
    // so cloning outer struct "State" should implicitly
    // share the db instances
    genesis_blk.set_state(state.clone());

    genesis_blk.verify().await.unwrap();
    assert!(state.has_verified(&genesis_blk.id()).await);

    genesis_blk.accept().await.unwrap();
    assert_eq!(genesis_blk.status, choices::status::Status::Accepted);
    assert!(state.has_last_accepted_block().await.unwrap());
    assert!(!state.has_verified(&genesis_blk.id()).await); // removed after acceptance

    let last_accepted_blk_id = state.get_last_accepted_block_id().await.unwrap();
    assert_eq!(last_accepted_blk_id, genesis_blk.id());

    let read_blk = state.get_block(&genesis_blk.id()).await.unwrap();
    assert_eq!(genesis_blk, read_blk);

    let action1 = tx::ActionType::CreateGame {
        white: Address::ZERO,
        black: Address::default(),
        block_id: ids::Id::default(),
    };
    let blk_tx = tx::Transaction {
        action: action1,
        bytes: Vec::new(),
        id: ids::Id::default(),
        size: 0,
        sender: Address::default(),
    };
    let mut blk1 = Block::try_new(
        genesis_blk.id,
        genesis_blk.height + 1,
        genesis_blk.timestamp + 1,
        String::from("first block!"),
        vec![blk_tx],
        choices::status::Status::default(),
    )
    .unwrap();

    log::info!("deserialized: {blk1} (block Id: {})", blk1.id);

    let serialized_blk1 = blk1.to_vec().unwrap();
    let deserialized_blk1 = Block::from_slice(serialized_blk1).unwrap();

    log::info!(
        "deserialized blk1: {deserialized_blk1} (block id: {})",
        deserialized_blk1.id
    )
}
