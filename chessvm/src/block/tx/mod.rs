use std::{
    fmt::Debug,
    io::{self, Error, ErrorKind},
};

use crate::{api::chain_handlers, state};
use alloy_primitives::Address;
use avalanche_types::ids;
use serde::{Deserialize, Serialize};
use shakmaty::{Move, Role, Square};

// pub mod action;
// pub mod create_game;
// pub mod end_game;
// pub mod make_move;

fn string_to_role(role: String) -> io::Result<Role> {
    // Convert role to char
    if let Some(role_char) = role.chars().next() {
        if let Some(r) = Role::from_char(role_char) {
            return Ok(r);
        }
    }

    Err(Error::new(
        ErrorKind::Other,
        "could not convert role to a Role!",
    ))
}

fn string_to_square(square: String) -> io::Result<Square> {
    // Convert string to byte ref
    let square_b = square.as_bytes();
    if let Ok(s) = Square::from_ascii(square_b) {
        return Ok(s);
    }
    Err(Error::new(
        ErrorKind::Other,
        "could not convert square to a Square!",
    ))
}

fn convert_normal_move(
    role: String,
    from: String,
    capture: Option<String>,
    to: String,
    promotion: Option<String>,
) -> io::Result<Move> {
    let capture_real = match capture {
        Some(s) => Some(string_to_role(s)?),
        None => None,
    };
    let promotion_real = match promotion {
        Some(s) => Some(string_to_role(s)?),
        None => None,
    };

    Ok(Move::Normal {
        role: string_to_role(role)?,
        from: string_to_square(from)?,
        capture: capture_real,
        to: string_to_square(to)?,
        promotion: promotion_real,
    })
}

fn convert_enpassant_move(from: String, to: String) -> io::Result<Move> {
    Ok(Move::EnPassant {
        from: string_to_square(from)?,
        to: string_to_square(to)?,
    })
}

fn convert_castle_move(king: String, rook: String) -> io::Result<Move> {
    Ok(Move::Castle {
        king: string_to_square(king)?,
        rook: string_to_square(rook)?,
    })
}

pub fn convert_move(mv: chain_handlers::MoveEnum) -> io::Result<Move> {
    match mv {
        chain_handlers::MoveEnum::Normal {
            role,
            from,
            capture,
            to,
            promotion,
        } => convert_normal_move(role, from, capture, to, promotion),
        chain_handlers::MoveEnum::EnPassant { from, to } => convert_enpassant_move(from, to),
        chain_handlers::MoveEnum::Castle { king, rook } => convert_castle_move(king, rook),
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ActionType {
    CreateGame {
        white: Address,
        black: Address,
        block_id: ids::Id,
    },
    EndGame {
        game_id: u64,
        block_id: ids::Id,
    },
    MakeMove {
        player: Address,
        game_id: u64,
        mv: chain_handlers::MoveEnum,
        block_id: ids::Id,
    },
    Unknown,
}

#[derive(Clone)]
pub struct TransactionContext {
    pub state: state::State,
    pub block_time: u64,
    pub tx_id: ids::Id,
    pub sender: Address,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
// #[derivative(Debug, PartialEq, Eq)]
pub struct Transaction {
    pub action: ActionType,

    #[serde(skip)]
    pub bytes: Vec<u8>,

    #[serde(skip)]
    pub id: ids::Id,

    #[serde(skip)]
    pub size: u64,

    #[serde(skip)]
    pub sender: Address,
}

impl Transaction {
    async fn get_block_id(&self) -> ids::Id {
        match &self.action {
            ActionType::Unknown => ids::Id::default(),
            ActionType::EndGame { game_id, block_id } => block_id.clone(),
            ActionType::MakeMove {
                player,
                game_id,
                mv,
                block_id,
            } => block_id.clone(),
            ActionType::CreateGame {
                white,
                black,
                block_id,
            } => block_id.clone(),
        }
    }

    async fn set_block_id(&mut self, id: ids::Id) {
        match &mut self.action {
            ActionType::CreateGame {
                white,
                black,
                block_id,
            } => *block_id = id,
            ActionType::EndGame { game_id, block_id } => *block_id = id,
            ActionType::MakeMove {
                player,
                game_id,
                mv,
                block_id,
            } => *block_id = id,
            _ => return,
        }
    }

    pub async fn execute(&self, tx_context: TransactionContext) -> io::Result<()> {
        match &self.action {
            ActionType::Unknown => Ok(()),
            ActionType::CreateGame {
                white,
                black,
                block_id,
            } => {
                create_game(tx_context, white.clone(), black.clone()).await?;
                Ok(())
            }
            ActionType::EndGame { game_id, block_id } => {
                end_game(tx_context, *game_id).await?;
                Ok(())
            }
            ActionType::MakeMove {
                player,
                game_id,
                mv,
                block_id,
            } => {
                make_move(tx_context, player.clone(), *game_id, mv.clone()).await?;
                Ok(())
            }
        }
    }

    async fn typ(&self) -> ActionType {
        self.action.clone()
    }
}

pub async fn create_game(
    tx_context: TransactionContext,
    white: Address,
    black: Address,
) -> io::Result<()> {
    // Create game
    tx_context.state.create_new_game(white, black).await?;

    Ok(())
}

pub async fn end_game(tx_context: TransactionContext, game_id: u64) -> io::Result<()> {
    tx_context.state.end_game(game_id).await?;

    Ok(())
}

pub async fn make_move(
    tx_context: TransactionContext,
    player: Address,
    game_id: u64,
    mv: chain_handlers::MoveEnum,
) -> io::Result<()> {
    let mv = convert_move(mv)?;
    tx_context.state.make_move(player, game_id, &mv).await?;

    Ok(())
}
