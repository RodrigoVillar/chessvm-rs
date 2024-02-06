use std::{
    fs,
    path::Path,
    str::FromStr,
    thread,
    time::{Duration, Instant},
};

use alloy_primitives::Address;
use avalanche_network_runner_sdk::{BlockchainSpec, Client, GlobalConfig, StartRequest};
use avalanche_types::{ids, jsonrpc::client::info as avalanche_sdk_info, subnet};

const AVALANCHEGO_VERSION: &str = "v1.10.9";

// #[tokio::test]
async fn e2e() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();

    let (ep, is_set) = crate::get_network_runner_grpc_endpoint();
    assert!(is_set);

    let cli = Client::new(&ep).await;

    log::info!("ping...");
    let resp = cli.ping().await.expect("failed ping");
    log::info!("network-runner is running (ping response {:?})", resp);

    log::info!("RUN THESE COMMANDS IF THE TESTS FAIL");
    log::info!("pkill -P {} || true", resp.pid);
    log::info!("kill -2 {} || true", resp.pid);

    let (vm_plugin_path, exists) = crate::get_vm_plugin_path();
    log::info!("Vm Plugin path: {vm_plugin_path}");
    assert!(exists);
    assert!(Path::new(&vm_plugin_path).exists());

    let vm_id = Path::new(&vm_plugin_path)
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let vm_id = subnet::vm_name_to_id(&vm_id).unwrap();

    let (mut avalanchego_exec_path, _) = crate::get_avalanchego_path();
    let plugins_dir = if !avalanchego_exec_path.is_empty() {
        let parent_dir = Path::new(&avalanchego_exec_path)
            .parent()
            .expect("unexpected None parent");
        parent_dir
            .join("plugins")
            .as_os_str()
            .to_str()
            .unwrap()
            .to_string()
    } else {
        let exec_path = avalanche_installer::avalanchego::github::download(
            None,
            None,
            Some(AVALANCHEGO_VERSION.to_string()),
        )
        .await
        .unwrap();
        avalanchego_exec_path = exec_path;
        avalanche_installer::avalanchego::get_plugin_dir(&avalanchego_exec_path)
    };

    log::info!(
        "copying vm plugin {} to {}/{}",
        vm_plugin_path,
        plugins_dir,
        vm_id
    );

    fs::create_dir(&plugins_dir).unwrap();
    fs::copy(
        &vm_plugin_path,
        Path::new(&plugins_dir).join(vm_id.to_string()),
    )
    .unwrap();

    // write some random genesis file
    let genesis = chessvm::genesis::Genesis {
        data: random_manager::secure_string(10),
    };
    let genesis_file_path = random_manager::tmp_path(10, None).unwrap();
    genesis.sync(&genesis_file_path).unwrap();

    log::info!(
        "starting {} with avalanchego {}, genesis file path {}",
        vm_id,
        &avalanchego_exec_path,
        genesis_file_path,
    );
    let resp = cli
        .start(StartRequest {
            exec_path: avalanchego_exec_path,
            num_nodes: Some(5),
            plugin_dir: plugins_dir,
            global_node_config: Some(
                serde_json::to_string(&GlobalConfig {
                    log_level: String::from("info"),
                })
                .unwrap(),
            ),
            blockchain_specs: vec![BlockchainSpec {
                vm_name: String::from("chessvm"),
                genesis: genesis_file_path.to_string(),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("failed start");
    log::info!(
        "started avalanchego cluster with network-runner: {:?}",
        resp
    );

    // enough time for network-runner to get ready
    thread::sleep(Duration::from_secs(20));

    log::info!("checking cluster healthiness...");
    let mut ready = false;

    let timeout = Duration::from_secs(300);
    let interval = Duration::from_secs(15);
    let start = Instant::now();
    let mut cnt: u128 = 0;
    loop {
        let elapsed = start.elapsed();
        if elapsed.gt(&timeout) {
            break;
        }

        let itv = {
            if cnt == 0 {
                // first poll with no wait
                Duration::from_secs(1)
            } else {
                interval
            }
        };
        thread::sleep(itv);

        ready = {
            match cli.health().await {
                Ok(_) => {
                    log::info!("healthy now!");
                    true
                }
                Err(e) => {
                    log::warn!("not healthy yet {}", e);
                    false
                }
            }
        };
        if ready {
            break;
        }

        cnt += 1;
    }
    assert!(ready);

    log::info!("checking status...");
    let mut status = cli.status().await.expect("failed status");
    loop {
        let elapsed = start.elapsed();
        if elapsed.gt(&timeout) {
            break;
        }

        if let Some(ci) = &status.cluster_info {
            if !ci.custom_chains.is_empty() {
                break;
            }
        }

        log::info!("retrying checking status...");
        thread::sleep(interval);
        status = cli.status().await.expect("failed status");
    }

    assert!(status.cluster_info.is_some());
    let cluster_info = status.cluster_info.unwrap();
    let mut rpc_eps: Vec<String> = Vec::new();
    for (node_name, iv) in cluster_info.node_infos.into_iter() {
        log::info!("{}: {}", node_name, iv.uri);
        rpc_eps.push(iv.uri.clone());
    }
    let mut blockchain_id = ids::Id::empty();
    for (k, v) in cluster_info.custom_chains.iter() {
        log::info!("custom chain info: {}={:?}", k, v);
        if v.chain_name == "chessvm" {
            blockchain_id = ids::Id::from_str(&v.chain_id).unwrap();
            break;
        }
    }
    log::info!("avalanchego RPC endpoints: {:?}", rpc_eps);

    let resp = avalanche_sdk_info::get_network_id(&rpc_eps[0])
        .await
        .unwrap();
    let network_id = resp.result.unwrap().network_id;
    log::info!("network Id: {}", network_id);

    log::info!("ping static handlers");
    let static_url_path = format!("ext/vm/{vm_id}/static");
    for ep in rpc_eps.iter() {
        let resp = chessvm::client::ping(ep.as_str(), &static_url_path)
            .await
            .unwrap();
        log::info!("ping response from {}: {:?}", ep, resp);
        assert!(resp.result.unwrap().success);

        thread::sleep(Duration::from_millis(300));
    }

    log::info!("ping chain handlers");
    let chain_url_path = format!("ext/bc/{blockchain_id}/rpc");
    for ep in rpc_eps.iter() {
        let resp = chessvm::client::ping(ep.as_str(), &chain_url_path)
            .await
            .unwrap();
        log::info!("ping response from {}: {:?}", ep, resp);
        assert!(resp.result.unwrap().success);

        thread::sleep(Duration::from_millis(300));
    }

    let ep = rpc_eps[0].clone();

    log::info!("get last_accepted from chain handlers");
    let resp = chessvm::client::last_accepted(&ep, &chain_url_path)
        .await
        .unwrap();
    log::info!("last_accepted response from {}: {:?}", ep, resp);

    // let blk_id = resp.result.unwrap().id;

    // log::info!("getting block {blk_id}");
    // let resp = chessvm::client::get_block(&ep, &chain_url_path, &blk_id)
    //     .await
    //     .unwrap();
    // log::info!("get_block response from {}: {:?}", ep, resp);
    // let height0 = resp.result.unwrap().block.height();

    log::info!("testing create_game method");
    let white = Address::ZERO;
    let addr_str = "0x66f9664f97F2b50F62D13eA064982f936dE76657";
    let black = Address::parse_checksummed(addr_str, None).unwrap();

    let resp = chessvm::client::create_game(&ep, &chain_url_path, white, black)
        .await
        .unwrap();

    let game_id = resp.result.unwrap().game_id;

    log::info!("Created a new game with game_id {}", game_id);

    // enough time for block builds
    thread::sleep(Duration::from_secs(5));

    log::info!("Testing get_game method!");
    let resp_2 = chessvm::client::get_game(&ep, &chain_url_path, game_id)
        .await
        .unwrap();
    log::info!("resp2 is {:?}", resp_2);
    log::info!("Current game is {}", resp_2.result.unwrap().game);

    log::info!("Testing make_move method!");
    let mv = chessvm::api::chain_handlers::MoveEnum::Normal {
        role: String::from("P"),
        from: String::from("e2"),
        capture: None,
        to: String::from("e4"),
        promotion: None,
    };
    let resp_3 = chessvm::client::make_move(&ep, &chain_url_path, white, game_id, mv)
        .await
        .unwrap();
    log::info!("resp3 is {:?}", resp_3);

    // enough time for block builds
    thread::sleep(Duration::from_secs(5));

    log::info!("Testing get_game method again!");
    let resp_4 = chessvm::client::get_game(&ep, &chain_url_path, game_id)
        .await
        .unwrap();
    log::info!("resp4 is {:?}", resp_4);
    log::info!("Current game is {}", resp_4.result.unwrap().game);

    // Now testing a chess game with an ending
    // Create new player
    let brown =
        Address::parse_checksummed("0x7f610402ccc4CC1BEbcE9699819200f5f28ED6e3", None).unwrap();
    // Create new game
    let resp5 = chessvm::client::create_game(&ep, &chain_url_path, white, brown)
        .await
        .unwrap();
    let game_id_2 = resp5.result.unwrap().game_id;
    log::info!("Created a new game with game_id {}", game_id_2);

    // Wait for block to be produced
    thread::sleep(Duration::from_secs(5));
    // Block should be produced now, make first move
    let mv = chessvm::api::chain_handlers::MoveEnum::Normal {
        role: String::from("P"),
        from: String::from("f2"),
        capture: None,
        to: String::from("f3"),
        promotion: None,
    };
    let resp6 = chessvm::client::make_move(&ep, &chain_url_path, white, game_id_2, mv)
        .await
        .unwrap();

    thread::sleep(Duration::from_secs(5));

    let mv = chessvm::api::chain_handlers::MoveEnum::Normal {
        role: String::from("p"),
        from: String::from("e7"),
        capture: None,
        to: String::from("e6"),
        promotion: None,
    };
    let resp7 = chessvm::client::make_move(&ep, &chain_url_path, brown, game_id_2, mv)
        .await
        .unwrap();

    thread::sleep(Duration::from_secs(5));

    let mv = chessvm::api::chain_handlers::MoveEnum::Normal {
        role: String::from("P"),
        from: String::from("g2"),
        capture: None,
        to: String::from("g4"),
        promotion: None,
    };
    let resp8 = chessvm::client::make_move(&ep, &chain_url_path, white, game_id_2, mv)
        .await
        .unwrap();

    thread::sleep(Duration::from_secs(5));

    let mv = chessvm::api::chain_handlers::MoveEnum::Normal {
        role: String::from("q"),
        from: String::from("d8"),
        capture: None,
        to: String::from("h4"),
        promotion: None,
    };
    let resp9 = chessvm::client::make_move(&ep, &chain_url_path, brown, game_id_2, mv)
        .await
        .unwrap();

    thread::sleep(Duration::from_secs(5));

    let resp_10 = chessvm::client::get_game(&ep, &chain_url_path, game_id_2)
        .await
        .unwrap();
    log::info!("resp_10 is {:?}", resp_10);
    log::info!("Current game is {}", resp_10.result.unwrap().game);

    // Testing if game is over
}

#[tokio::test]
async fn start_network() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();

    let (ep, is_set) = crate::get_network_runner_grpc_endpoint();
    assert!(is_set);

    let cli = Client::new(&ep).await;

    log::info!("ping...");
    let resp = cli.ping().await.expect("failed ping");
    log::info!("network-runner is running (ping response {:?})", resp);

    log::info!("RUN THESE COMMANDS IF THE TESTS FAIL");
    log::info!("pkill -P {} || true", resp.pid);
    log::info!("kill -2 {} || true", resp.pid);

    let (vm_plugin_path, exists) = crate::get_vm_plugin_path();
    log::info!("Vm Plugin path: {vm_plugin_path}");
    assert!(exists);
    assert!(Path::new(&vm_plugin_path).exists());

    let vm_id = Path::new(&vm_plugin_path)
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let vm_id = subnet::vm_name_to_id(&vm_id).unwrap();

    let (mut avalanchego_exec_path, _) = crate::get_avalanchego_path();
    let plugins_dir = if !avalanchego_exec_path.is_empty() {
        let parent_dir = Path::new(&avalanchego_exec_path)
            .parent()
            .expect("unexpected None parent");
        parent_dir
            .join("plugins")
            .as_os_str()
            .to_str()
            .unwrap()
            .to_string()
    } else {
        let exec_path = avalanche_installer::avalanchego::github::download(
            None,
            None,
            Some(AVALANCHEGO_VERSION.to_string()),
        )
        .await
        .unwrap();
        avalanchego_exec_path = exec_path;
        avalanche_installer::avalanchego::get_plugin_dir(&avalanchego_exec_path)
    };

    log::info!(
        "copying vm plugin {} to {}/{}",
        vm_plugin_path,
        plugins_dir,
        vm_id
    );

    fs::create_dir(&plugins_dir).unwrap();
    fs::copy(
        &vm_plugin_path,
        Path::new(&plugins_dir).join(vm_id.to_string()),
    )
    .unwrap();

    // write some random genesis file
    let genesis = chessvm::genesis::Genesis {
        data: random_manager::secure_string(10),
    };
    let genesis_file_path = random_manager::tmp_path(10, None).unwrap();
    genesis.sync(&genesis_file_path).unwrap();

    log::info!(
        "starting {} with avalanchego {}, genesis file path {}",
        vm_id,
        &avalanchego_exec_path,
        genesis_file_path,
    );
    let resp = cli
        .start(StartRequest {
            exec_path: avalanchego_exec_path,
            num_nodes: Some(5),
            plugin_dir: plugins_dir,
            global_node_config: Some(
                serde_json::to_string(&GlobalConfig {
                    log_level: String::from("info"),
                })
                .unwrap(),
            ),
            blockchain_specs: vec![BlockchainSpec {
                vm_name: String::from("chessvm"),
                genesis: genesis_file_path.to_string(),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("failed start");
    log::info!(
        "started avalanchego cluster with network-runner: {:?}",
        resp
    );

    // enough time for network-runner to get ready
    thread::sleep(Duration::from_secs(20));

    log::info!("checking cluster healthiness...");
    let mut ready = false;

    let timeout = Duration::from_secs(300);
    let interval = Duration::from_secs(15);
    let start = Instant::now();
    let mut cnt: u128 = 0;
    loop {
        let elapsed = start.elapsed();
        if elapsed.gt(&timeout) {
            break;
        }

        let itv = {
            if cnt == 0 {
                // first poll with no wait
                Duration::from_secs(1)
            } else {
                interval
            }
        };
        thread::sleep(itv);

        ready = {
            match cli.health().await {
                Ok(_) => {
                    log::info!("healthy now!");
                    true
                }
                Err(e) => {
                    log::warn!("not healthy yet {}", e);
                    false
                }
            }
        };
        if ready {
            break;
        }

        cnt += 1;
    }
    assert!(ready);

    log::info!("checking status...");
    let mut status = cli.status().await.expect("failed status");
    loop {
        let elapsed = start.elapsed();
        if elapsed.gt(&timeout) {
            break;
        }

        if let Some(ci) = &status.cluster_info {
            if !ci.custom_chains.is_empty() {
                break;
            }
        }

        log::info!("retrying checking status...");
        thread::sleep(interval);
        status = cli.status().await.expect("failed status");
    }

    assert!(status.cluster_info.is_some());
    let cluster_info = status.cluster_info.unwrap();
    let mut rpc_eps: Vec<String> = Vec::new();
    for (node_name, iv) in cluster_info.node_infos.into_iter() {
        log::info!("{}: {}", node_name, iv.uri);
        rpc_eps.push(iv.uri.clone());
    }
    let mut blockchain_id = ids::Id::empty();
    for (k, v) in cluster_info.custom_chains.iter() {
        log::info!("custom chain info: {}={:?}", k, v);
        if v.chain_name == "chessvm" {
            blockchain_id = ids::Id::from_str(&v.chain_id).unwrap();
            break;
        }
    }
    log::info!("avalanchego RPC endpoints: {:?}", rpc_eps);

    let resp = avalanche_sdk_info::get_network_id(&rpc_eps[0])
        .await
        .unwrap();
    let network_id = resp.result.unwrap().network_id;
    log::info!("network Id: {}", network_id);

    log::info!("ping static handlers");
    let static_url_path = format!("ext/vm/{vm_id}/static");
    for ep in rpc_eps.iter() {
        let resp = chessvm::client::ping(ep.as_str(), &static_url_path)
            .await
            .unwrap();
        log::info!("ping response from {}: {:?}", ep, resp);
        assert!(resp.result.unwrap().success);

        thread::sleep(Duration::from_millis(300));
    }

    log::info!("ping chain handlers");
    let chain_url_path = format!("ext/bc/{blockchain_id}/rpc");
    for ep in rpc_eps.iter() {
        let resp = chessvm::client::ping(ep.as_str(), &chain_url_path)
            .await
            .unwrap();
        log::info!("ping response from {}: {:?}", ep, resp);
        assert!(resp.result.unwrap().success);

        thread::sleep(Duration::from_millis(300));
    }

    let ep = rpc_eps[0].clone();

    println!("\n----\n");

    println!("Recommended HTTP-RPC: {}", ep);
    println!("Recommended URL-Path: {}", chain_url_path);
}
