//! Implements client for ChessVM APIs.

use std::{
    collections::HashMap,
    io::{self, Error, ErrorKind},
};

use alloy_primitives::Address;
use avalanche_types::{ids, jsonrpc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::chain_handlers;

pub fn move_enum_to_json_string(mv: chain_handlers::MoveEnum) -> io::Result<String> {
    serde_json::to_string(&mv).map_err(|e| {
        Error::new(
            ErrorKind::Other,
            format!("failed to serialize MoveEnum to JSON string {e}"),
        )
    })
}

/// Represents the RPC response for API `ping`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PingResponse {
    pub jsonrpc: String,
    pub id: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<crate::api::PingResponse>,

    /// Returns non-empty if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<APIError>,
}

/// Ping the VM.
/// # Errors
/// Errors on an http failure or a failed deserialization.
pub async fn ping(http_rpc: &str, url_path: &str) -> io::Result<PingResponse> {
    log::info!("ping {http_rpc} with {url_path}");

    let mut data = jsonrpc::RequestWithParamsArray::default();
    data.method = String::from("chessvm.ping");

    let d = data.encode_json()?;
    log::info!("{}", d);
    let rb = http_manager::post_non_tls(http_rpc, url_path, &d).await?;

    serde_json::from_slice(&rb)
        .map_err(|e| Error::new(ErrorKind::Other, format!("failed ping '{e}'")))
}

/// Represents the RPC response for API `createGame`
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateGameResponse {
    pub jsonrpc: String,
    pub id: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<crate::api::chain_handlers::CreateGameResponse>,

    /// Returns non-empty if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<APIError>,
}

/// Sends a TX to create a new chess game
pub async fn create_game(
    http_rpc: &str,
    url_path: &str,
    white: Address,
    black: Address,
) -> io::Result<CreateGameResponse> {
    log::info!("create_game method to {http_rpc} with {url_path}");

    let mut data = jsonrpc::RequestWithParamsHashMapArray::default();
    data.method = String::from("chessvm.createGame");

    let mut m = HashMap::new();
    m.insert("white".to_string(), white.to_string());
    m.insert("black".to_string(), black.to_string());

    let params = vec![m];
    data.params = Some(params);

    let d = data.encode_json()?;
    log::info!("{}", d);
    let rb = http_manager::post_non_tls(http_rpc, url_path, &d).await?;

    serde_json::from_slice(&rb)
        .map_err(|e| Error::new(ErrorKind::Other, format!("failed create_game '{e}'")))
}

/// Represents the RPC response for API `getGame`
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GetGameResponse {
    pub jsonrpc: String,
    pub id: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<crate::api::chain_handlers::GetGameResponse>,

    /// Returns non-empty if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<APIError>,
}

/// Requests the current state of a Chess Game
pub async fn get_game(http_rpc: &str, url_path: &str, game_id: u64) -> io::Result<GetGameResponse> {
    log::info!("get_game method {http_rpc} with {url_path}");

    let mut data = jsonrpc::RequestWithParamsHashMapArray::default();

    data.method = String::from("getGame");

    let mut m = HashMap::new();
    m.insert("game_id".to_string(), game_id.to_string());

    let params = vec![m];
    data.params = Some(params);

    let d = data.encode_json()?;
    log::info!("{}", d);
    let rb = http_manager::post_non_tls(http_rpc, url_path, &d).await?;

    serde_json::from_slice(&rb)
        .map_err(|e| Error::new(ErrorKind::Other, format!("failed get_game '{e}'")))
}

/// Requests for the last accepted block Id.
/// # Errors
/// Errors on failed (de)serialization or an http failure.
pub async fn last_accepted(http_rpc: &str, url_path: &str) -> io::Result<LastAcceptedResponse> {
    log::info!("last_accepted {http_rpc} with {url_path}");

    let mut data = jsonrpc::RequestWithParamsArray::default();
    data.method = String::from("chessvm.lastAccepted");

    let d = data.encode_json()?;
    let rb = http_manager::post_non_tls(http_rpc, url_path, &d).await?;

    serde_json::from_slice(&rb)
        .map_err(|e| Error::new(ErrorKind::Other, format!("failed last_accepted '{e}'")))
}

/// Represents the RPC response for API `last_accepted`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LastAcceptedResponse {
    pub jsonrpc: String,
    pub id: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<crate::api::chain_handlers::LastAcceptedResponse>,

    /// Returns non-empty if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<APIError>,
}

/// Represents the RPC response for API `get_block`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GetBlockResponse {
    pub jsonrpc: String,
    pub id: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<crate::api::chain_handlers::GetBlockResponse>,

    /// Returns non-empty if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<APIError>,
}

/// Fetches the block for the corresponding block Id (if any).
/// # Errors
/// Errors on failed (de)serialization or an http failure.
pub async fn get_block(
    http_rpc: &str,
    url_path: &str,
    id: &ids::Id,
) -> io::Result<GetBlockResponse> {
    log::info!("get_block {http_rpc} with {url_path}");

    let mut data = jsonrpc::RequestWithParamsHashMapArray::default();
    data.method = String::from("chessvm.getBlock");

    let mut m = HashMap::new();
    m.insert("id".to_string(), id.to_string());

    let params = vec![m];
    data.params = Some(params);

    let d = data.encode_json()?;
    let rb = http_manager::post_non_tls(http_rpc, url_path, &d).await?;

    serde_json::from_slice(&rb)
        .map_err(|e| Error::new(ErrorKind::Other, format!("failed get_block '{e}'")))
}

/// Represents the RPC response for API `make_move`
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MakeMoveResponse {
    pub jsonrpc: String,
    pub id: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<crate::api::chain_handlers::MakeMoveResponse>,

    /// Returns non-empty if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<APIError>,
}

/// Makes a move for a given Chess game
pub async fn make_move(
    http_rpc: &str,
    url_path: &str,
    player: Address,
    game_id: u64,
    mv: chain_handlers::MoveEnum,
) -> io::Result<MakeMoveResponse> {
    log::info!("make_move {http_rpc} with {url_path}");

    let mut data = jsonrpc::RequestWithParamsHashMapArray::default();
    data.method = String::from("chessvm.makeMove");

    let mut m = HashMap::new();
    // Inserting player arg
    m.insert("player".to_string(), player.to_string());
    // Inserting game_id
    m.insert("game_id".to_string(), game_id.to_string());

    let params = vec![m];
    data.params = Some(params);

    let d = data.encode_json()?;

    // Need to add mv to data
    // adding mv
    let mut d_json: Value = serde_json::from_slice(d.as_bytes()).unwrap();
    let mv_json: Value =
        serde_json::from_slice(move_enum_to_json_string(mv).unwrap().as_bytes()).unwrap();
    let val = d_json["params"].get_mut(0).unwrap();
    val["mv"] = mv_json;

    // Serialize back to JSON string
    let d = serde_json::to_string(&d_json).unwrap();

    log::info!("{}", d);
    let rb = http_manager::post_non_tls(http_rpc, url_path, &d).await?;

    serde_json::from_slice(&rb)
        .map_err(|e| Error::new(ErrorKind::Other, format!("failed make_move '{e}'")))
}

/// Represents the RPC response for API `exists`
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExistsResponse {
    pub jsonrpc: String,
    pub id: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<crate::api::chain_handlers::ExistsResponse>,

    /// Returns non-empty if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<APIError>,
}

/// Checks if a game exists
pub async fn exists(http_rpc: &str, url_path: &str, game_id: u64) -> io::Result<ExistsResponse> {
    log::info!("exists method {http_rpc} with {url_path}");

    let mut data = jsonrpc::RequestWithParamsHashMapArray::default();

    data.method = String::from("chessvm.exists");

    let mut m = HashMap::new();
    m.insert("game_id".to_string(), game_id.to_string());

    let params = vec![m];
    data.params = Some(params);

    let d = data.encode_json()?;
    log::info!("{}", d);
    let rb = http_manager::post_non_tls(http_rpc, url_path, &d).await?;

    serde_json::from_slice(&rb)
        .map_err(|e| Error::new(ErrorKind::Other, format!("failed get_game '{e}'")))
}

/// Represents the error (if any) for APIs.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct APIError {
    pub code: i32,
    pub message: String,
}

#[tokio::test]
async fn test_client() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();

    let random_mv = chain_handlers::MoveEnum::Normal {
        role: String::from("P"),
        from: String::from("e2"),
        capture: None,
        to: String::from("e4"),
        promotion: None,
    };

    let mut data = jsonrpc::RequestWithParamsHashMapArray::default();
    data.method = String::from("chessvm.makeMove");
    let player = Address::default();
    let game_id = 0;
    let mut m = HashMap::new();
    // Inserting player arg
    m.insert("player".to_string(), player.to_string());
    // Inserting game_id
    m.insert("game_id".to_string(), game_id.to_string());

    let params = vec![m];
    data.params = Some(params);

    let d = data.encode_json().unwrap();

    // adding mv
    let mut d_json: Value = serde_json::from_slice(d.as_bytes()).unwrap();
    let mv_json: Value =
        serde_json::from_slice(move_enum_to_json_string(random_mv).unwrap().as_bytes()).unwrap();
    let val = d_json["params"].get_mut(0).unwrap();
    val["mv"] = mv_json;

    // Serialize back to JSON string
    let modified_json_str = serde_json::to_string(&d_json).unwrap();
    println!("{}", modified_json_str);
}
