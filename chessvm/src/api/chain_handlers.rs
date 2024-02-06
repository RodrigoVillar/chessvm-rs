//! Implements chain/VM specific handlers.
//! To be served via `[HOST]/ext/bc/[CHAIN ID]/rpc`.

use crate::{
    block::{
        tx::{self, ActionType, Transaction, TransactionContext},
        Block,
    },
    state::calculate_game_id,
    vm::Vm,
};
use avalanche_types::{ids, proto::http::Element, subnet::rpc::http::handle::Handle};
use bytes::Bytes;
use jsonrpc_core::{BoxFuture, Error, ErrorCode, IoHandler, Result};
use jsonrpc_derive::rpc;
use serde::{Deserialize, Serialize};
use shakmaty::{Chess, Position};
use std::{borrow::Borrow, fmt::Debug, io, marker::PhantomData, str::FromStr};

use alloy_primitives::Address;

use super::de_request;

/// Defines RPCs specific to the chain.
#[rpc]
pub trait Rpc {
    /// Pings the VM.
    #[rpc(name = "ping", alias("chessvm.ping"))]
    fn ping(&self) -> BoxFuture<Result<crate::api::PingResponse>>;

    /// Fetches the last accepted block.
    #[rpc(name = "lastAccepted", alias("chessvm.lastAccepted"))]
    fn last_accepted(&self) -> BoxFuture<Result<LastAcceptedResponse>>;

    /// Fetches the block.
    #[rpc(name = "getBlock", alias("chessvm.getBlock"))]
    fn get_block(&self, args: GetBlockArgs) -> BoxFuture<Result<GetBlockResponse>>;

    // RPCs specific to ChessVM
    /// Creates new Chess game
    #[rpc(name = "createGame", alias("chessvm.createGame"))]
    fn create_game(&self, args: CreateGameArgs) -> BoxFuture<Result<CreateGameResponse>>;

    /// Make a Chess move
    #[rpc(name = "makeMove", alias("chessvm.makeMove"))]
    fn make_move(&self, args: MakeMoveArgs) -> BoxFuture<Result<MakeMoveResponse>>;

    /// End a Chess game
    #[rpc(name = "endGame", alias("chessvm.endGame"))]
    fn end_game(&self, args: EndGameArgs) -> BoxFuture<Result<EndGameResponse>>;

    /// Get Chess game state
    #[rpc(name = "getGame", alias("chessvm.getGame"))]
    fn get_game(&self, args: GetGameArgs) -> BoxFuture<Result<GetGameResponse>>;

    /// Check if a game exists
    #[rpc(name = "exists", alias("chessvm.exists"))]
    fn exists(&self, args: ExistsArgs) -> BoxFuture<Result<ExistsResponse>>;
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LastAcceptedResponse {
    pub id: ids::Id,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GetBlockArgs {
    /// TODO: use "ids::Id"
    /// if we use "ids::Id", it fails with:
    /// "Invalid params: invalid type: string \"g25v3qDyAaHfR7kBev8tLUHouSgN5BJuZjy1BYS1oiHd2vres\", expected a borrowed string."
    pub id: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GetBlockResponse {
    pub block: Block,
}

// Specific to ChessVM
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CreateGameArgs {
    white: Address,
    black: Address,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CreateGameResponse {
    pub game_id: u64,
}

/// We need to implement this since the Move enum from the chess package cannot
/// be serialized :(
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MoveEnum {
    Normal {
        role: String,
        from: String,
        capture: Option<String>,
        to: String,
        promotion: Option<String>,
    },
    EnPassant {
        from: String,
        to: String,
    },
    Castle {
        king: String,
        rook: String,
    },
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct MakeMoveArgs {
    player: Address,
    game_id: String,
    mv: MoveEnum,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct MakeMoveResponse {
    pub status: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct EndGameArgs {
    game_id: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct EndGameResponse {
    status: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GetGameArgs {
    pub game_id: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GetGameResponse {
    pub game: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ExistsArgs {
    pub game_id: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ExistsResponse {
    pub exists: bool,
}

/// Implements API services for the chain-specific handlers.
#[derive(Clone)]
pub struct ChainService<A> {
    pub vm: Vm<A>,
}

impl<A> ChainService<A> {
    pub fn new(vm: Vm<A>) -> Self {
        Self { vm }
    }
}

impl<A> Rpc for ChainService<A>
where
    A: Send + Sync + Clone + 'static,
{
    #[doc = r" Pings the VM."]
    fn ping(&self) -> BoxFuture<Result<crate::api::PingResponse>> {
        log::debug!("ping called");
        Box::pin(async move { Ok(crate::api::PingResponse { success: true }) })
    }

    #[doc = r" Fetches the last accepted block."]
    fn last_accepted(&self) -> BoxFuture<Result<LastAcceptedResponse>> {
        log::debug!("last accept method called!");
        let vm = self.vm.clone();

        Box::pin(async move {
            let vm_state = vm.vm_state.read().await;
            if let Some(state) = &vm_state.state {
                let last_accepted = state
                    .get_last_accepted_block_id()
                    .await
                    .map_err(create_jsonrpc_error)?;

                return Ok(LastAcceptedResponse { id: last_accepted });
            }

            Err(Error {
                code: ErrorCode::InternalError,
                message: String::from("No state manager found"),
                data: None,
            })
        })
    }

    #[doc = r" Fetches the block."]
    fn get_block(&self, args: GetBlockArgs) -> BoxFuture<Result<GetBlockResponse>> {
        let blk_id = ids::Id::from_str(&args.id).unwrap();
        log::info!("get_block called for {}", blk_id);

        let vm = self.vm.clone();

        Box::pin(async move {
            let vm_state = vm.vm_state.read().await;
            if let Some(state) = &vm_state.state {
                let block = state
                    .get_block(&blk_id)
                    .await
                    .map_err(create_jsonrpc_error)?;

                return Ok(GetBlockResponse { block });
            }

            Err(Error {
                code: ErrorCode::InternalError,
                message: String::from("no state manager found"),
                data: None,
            })
        })
    }

    #[doc = r" Creates new Chess game"]
    /// Write method
    fn create_game(&self, args: CreateGameArgs) -> BoxFuture<Result<CreateGameResponse>> {
        log::debug!("create_game API method called!");
        let vm = self.vm.clone();

        Box::pin(async move {
            let act = ActionType::CreateGame {
                white: args.white,
                black: args.black,
                block_id: ids::Id::empty(),
            };
            let tx = Transaction {
                action: act,
                bytes: Vec::new(),
                id: ids::Id::empty(),
                size: 0,
                sender: args.white,
            };
            vm.submit_tx(tx).await.map_err(create_jsonrpc_error)?;
            Ok(CreateGameResponse {
                game_id: calculate_game_id(args.white, args.black),
            })
        })
    }

    #[doc = r" Make a Chess move"]
    /// Write method
    fn make_move(&self, args: MakeMoveArgs) -> BoxFuture<Result<MakeMoveResponse>> {
        log::debug!("make_move method called");
        let vm = self.vm.clone();

        Box::pin(async move {
            let vm_state = vm.vm_state.read().await;
            if let Some(_) = &vm_state.state {
                // Create TX and send to mempool
                // TODO: fix block_id
                let act = ActionType::MakeMove {
                    player: args.player,
                    game_id: args.game_id.parse::<u64>().unwrap(),
                    mv: args.mv,
                    block_id: ids::Id::empty(),
                };
                let tx = Transaction {
                    action: act,
                    bytes: Vec::new(),
                    id: ids::Id::empty(),
                    size: 0,
                    sender: args.player,
                };
                let r_val = vm.submit_tx(tx).await;
                if r_val.is_err() {
                    return Err(Error {
                        code: ErrorCode::InternalError,
                        message: String::from("Submitting make move transaction failed!"),
                        data: None,
                    });
                }

                return Ok(MakeMoveResponse { status: true });
            }

            Err(Error {
                code: ErrorCode::InternalError,
                message: String::from("no state manager found"),
                data: None,
            })
        })
    }

    #[doc = r" End a Chess game"]
    /// Write method
    fn end_game(&self, args: EndGameArgs) -> BoxFuture<Result<EndGameResponse>> {
        log::debug!("end_game method called");
        let vm = self.vm.clone();

        Box::pin(async move {
            let vm_state = vm.vm_state.write().await;
            if let Some(_) = &vm_state.state {
                // Create TX and submit to mempool
                // Can set block_id to 0 since never used
                // TODO: Fix block_id
                let act = ActionType::EndGame {
                    game_id: args.game_id,
                    block_id: ids::Id::empty(),
                };
                let tx = Transaction {
                    action: act,
                    bytes: Vec::new(),
                    id: ids::Id::empty(),
                    size: 0,
                    sender: Address::default(),
                };
                let r_val = vm.submit_tx(tx).await;
                if r_val.is_err() {
                    return Err(Error {
                        code: ErrorCode::InternalError,
                        message: String::from("Submitting end game transaction failed!"),
                        data: None,
                    });
                }
                return Ok(EndGameResponse { status: true });
            }

            Err(Error {
                code: ErrorCode::InternalError,
                message: String::from("no state manager found"),
                data: None,
            })
        })
    }

    #[doc = r"Get Chess game state"]
    /// Read method
    fn get_game(&self, args: GetGameArgs) -> BoxFuture<Result<GetGameResponse>> {
        log::debug!("get_game method called!");
        let vm = self.vm.clone();

        Box::pin(async move {
            let vm_state = vm.vm_state.read().await;
            if let Some(state) = &vm_state.state {
                if let Some(game) = state.get_game(args.game_id.parse::<u64>().unwrap()).await {
                    // TODO: Convert Chess board to string
                    return Ok(GetGameResponse {
                        game: game.board().to_string(),
                    });
                }
                log::info!("Game was NOT found in state :(");
            }

            Err(Error {
                code: ErrorCode::InternalError,
                message: String::from("no state manager found"),
                data: None,
            })
        })
    }

    #[doc = r"Check if game exists"]
    /// Read method
    fn exists(&self, args: ExistsArgs) -> BoxFuture<Result<ExistsResponse>> {
        log::debug!("exists method called!");
        let vm = self.vm.clone();

        Box::pin(async move {
            let vm_state = vm.vm_state.read().await;

            if let Some(state) = &vm_state.state {
                return Ok(ExistsResponse {
                    exists: state
                        .game_exists(args.game_id.parse::<u64>().unwrap())
                        .await,
                });
            }

            Err(Error {
                code: ErrorCode::InternalError,
                message: String::from("no state manager found"),
                data: None,
            })
        })
    }
}

#[derive(Clone, Debug)]
pub struct ChainHandler<T> {
    pub handler: IoHandler,
    _marker: PhantomData<T>,
}

impl<T: Rpc> ChainHandler<T> {
    pub fn new(service: T) -> Self {
        let mut handler = jsonrpc_core::IoHandler::new();
        handler.extend_with(Rpc::to_delegate(service));
        Self {
            handler,
            _marker: PhantomData,
        }
    }
}

#[tonic::async_trait]
impl<T> Handle for ChainHandler<T>
where
    T: Rpc + Send + Sync + Clone + 'static,
{
    async fn request(
        &self,
        req: &Bytes,
        _headers: &[Element],
    ) -> std::io::Result<(Bytes, Vec<Element>)> {
        match self.handler.handle_request(&de_request(req)?).await {
            Some(resp) => Ok((Bytes::from(resp), Vec::new())),
            None => Err(io::Error::new(
                io::ErrorKind::Other,
                "failed to handle request",
            )),
        }
    }
}

fn create_jsonrpc_error<E: Borrow<std::io::Error>>(e: E) -> Error {
    let e = e.borrow();
    let mut error = Error::new(ErrorCode::InternalError);
    error.message = format!("{e}");
    error
}

#[tokio::test]
async fn test_chess() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();

    let game = Chess::default();
    let game_string = game.board().to_string();

    log::info!("{}", game_string);
}
