
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

use monitoring_rust::config::Config;
use monitoring_rust::geth::client::GethClient;
use monitoring_rust::monitoring::scheduler::MonitoringScheduler;

use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

// Read an HTTP request and extract its JSON body.
async fn read_request(socket: &mut TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];

    loop {
        let n = socket.read(&mut chunk).await.unwrap();

        if n == 0 {
            return String::new();
        }

        buffer.extend_from_slice(&chunk[..n]);

        if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&buffer[..pos]).to_lowercase();

            let content_length = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(0);

            let body_start = pos + 4;

            if buffer.len() >= body_start + content_length {
                return String::from_utf8_lossy(
                    &buffer[body_start..body_start + content_length],
                )
                .into_owned();
            }
        }
    }
}

// Fake Geth JSON-RPC server.
async fn spawn_fake_geth() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };

            tokio::spawn(async move {
                let body = read_request(&mut socket).await;
                let request: Value = serde_json::from_str(&body).unwrap();

                let result = match request["method"].as_str().unwrap() {
                    "eth_blockNumber" => json!("0x1b4"),
                    "eth_syncing" => json!(false),
                    "net_peerCount" => json!("0xa"),
                    "net_listening" => json!(true),
                    "admin_peers" => json!([
                        { "id": "peer-1" },
                        { "id": "peer-2" }
                    ]),
                    method => panic!("Unexpected RPC method: {method}"),
                };

                let response = json!({
                    "jsonrpc": "2.0",
                    "id": request["id"],
                    "result": result
                })
                .to_string();

                let http_response = format!(
                    "HTTP/1.1 200 OK\r\n\
                     Content-Type: application/json\r\n\
                     Content-Length: {}\r\n\
                     Connection: close\r\n\r\n{}",
                    response.len(),
                    response
                );

                let _ = socket.write_all(http_response.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });

    format!("http://{address}")
}

// Fake Geth server without the admin_peers RPC method.
async fn spawn_fake_geth_without_admin() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };

            tokio::spawn(async move {
                let body = read_request(&mut socket).await;
                let request: Value = serde_json::from_str(&body).unwrap();

                let response = match request["method"].as_str().unwrap() {
                    "eth_blockNumber" => json!({
                        "jsonrpc": "2.0",
                        "id": request["id"],
                        "result": "0x1b4"
                    }),
                    "eth_syncing" => json!({
                        "jsonrpc": "2.0",
                        "id": request["id"],
                        "result": false
                    }),
                    "net_peerCount" => json!({
                        "jsonrpc": "2.0",
                        "id": request["id"],
                        "result": "0xa"
                    }),
                    "net_listening" => json!({
                        "jsonrpc": "2.0",
                        "id": request["id"],
                        "result": true
                    }),
                    "admin_peers" => json!({
                        "jsonrpc": "2.0",
                        "id": request["id"],
                        "error": {
                            "code": -32601,
                            "message": "the method admin_peers does not exist/is not available"
                        }
                    }),
                    method => panic!("Unexpected RPC method: {method}"),
                };

                let response = response.to_string();

                let http_response = format!(
                    "HTTP/1.1 200 OK\r\n\
                     Content-Type: application/json\r\n\
                     Content-Length: {}\r\n\
                     Connection: close\r\n\r\n{}",
                    response.len(),
                    response
                );

                let _ = socket.write_all(http_response.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });

    format!("http://{address}")
}

// Fake Geth server that replaces 50 peers between two collection cycles.
async fn spawn_fake_geth_peer_rotation() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();

    let admin_calls = Arc::new(AtomicUsize::new(0));

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };

            let admin_calls = Arc::clone(&admin_calls);

            tokio::spawn(async move {
                let body = read_request(&mut socket).await;
                let request: Value = serde_json::from_str(&body).unwrap();

                let method = request["method"].as_str().unwrap();

                let result = match method {
                    "eth_blockNumber" => json!("0x1b4"),
                    "eth_syncing" => json!(false),
                    "net_peerCount" => json!("0x32"),
                    "net_listening" => json!(true),

                    "admin_peers" => {
                        let cycle = admin_calls.fetch_add(1, Ordering::SeqCst);

                        let prefix = if cycle == 0 { "A" } else { "B" };

                        let peers: Vec<Value> = (1..=50)
                            .map(|n| {
                                json!({
                                    "id": format!("{prefix}{n}")
                                })
                            })
                            .collect();

                        json!(peers)
                    }

                    method => panic!("Unexpected RPC method: {method}"),
                };

                let response = json!({
                    "jsonrpc": "2.0",
                    "id": request["id"],
                    "result": result
                })
                .to_string();

                let http_response = format!(
                    "HTTP/1.1 200 OK\r\n\
                     Content-Type: application/json\r\n\
                     Content-Length: {}\r\n\
                     Connection: close\r\n\r\n{}",
                    response.len(),
                    response
                );

                let _ = socket.write_all(http_response.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });

    format!("http://{address}")
}

// Test 1: Normal collection and JSONL persistence.
#[tokio::test]
async fn collect_once_creates_and_persists_observation() {
    let rpc_url = spawn_fake_geth().await;

    let output_file = std::env::temp_dir().join(format!(
        "eva_guard_scheduler_test_{}.jsonl",
        std::process::id()
    ));

    let _ = std::fs::remove_file(&output_file);

    let config = Config {
        rpc_url,
        rpc_timeout: Duration::from_secs(2),
        collection_interval: Duration::from_secs(5),
        output_file: output_file.clone(),
    };

    let client = GethClient::new(&config).unwrap();

    let mut scheduler =
        MonitoringScheduler::new(&client, &config, "test-node").unwrap();

    let observation = scheduler.collect_once().await.unwrap();

    assert_eq!(observation.node_id, "test-node");
    assert_eq!(observation.collection_interval, 5);

    assert_eq!(observation.blockchain.block_number, Some(436));
    assert_eq!(observation.blockchain.is_synced, Some(true));

    assert_eq!(observation.p2p.peer_count, Some(10));
    assert_eq!(observation.p2p.admin_peer_count, Some(2));
    assert_eq!(observation.p2p.network_listening, Some(true));

    assert_eq!(observation.rpc.rpc_available, Some(true));
    assert!(observation.rpc.rpc_latency_ms.unwrap() >= 0.0);

    let content = std::fs::read_to_string(&output_file).unwrap();
    let saved: Value = serde_json::from_str(content.trim()).unwrap();

    assert_eq!(saved["node_id"], "test-node");
    assert_eq!(saved["block_number"], 436);
    assert_eq!(saved["peer_count"], 10);

    let _ = std::fs::remove_file(output_file);
}

// Test 2: Scheduler continues when Geth is unavailable.
#[tokio::test]
async fn scheduler_continues_when_geth_is_unavailable() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);

    let output_file = std::env::temp_dir().join(format!(
        "eva_guard_scheduler_unavailable_{}.jsonl",
        std::process::id()
    ));

    let _ = std::fs::remove_file(&output_file);

    let config = Config {
        rpc_url: format!("http://{address}"),
        rpc_timeout: Duration::from_secs(2),
        collection_interval: Duration::from_secs(5),
        output_file: output_file.clone(),
    };

    let client = GethClient::new(&config).unwrap();

    let mut scheduler =
        MonitoringScheduler::new(&client, &config, "test-node-unavailable")
            .unwrap();

    let observation = scheduler.collect_once().await.unwrap();

    assert_eq!(observation.blockchain.block_number, None);
    assert_eq!(observation.blockchain.is_synced, None);
    assert_eq!(observation.p2p.peer_count, None);
    assert_eq!(observation.rpc.rpc_available, Some(false));

    let content = std::fs::read_to_string(&output_file).unwrap();
    let saved: Value = serde_json::from_str(content.trim()).unwrap();

    assert_eq!(saved["node_id"], "test-node-unavailable");
    assert_eq!(saved["rpc_available"], false);

    let _ = std::fs::remove_file(output_file);
}

// Test 3: Scheduler handles a disabled admin namespace.
#[tokio::test]
async fn scheduler_handles_disabled_admin_namespace() {
    let rpc_url = spawn_fake_geth_without_admin().await;

    let output_file = std::env::temp_dir().join(format!(
        "eva_guard_scheduler_no_admin_{}.jsonl",
        std::process::id()
    ));

    let _ = std::fs::remove_file(&output_file);

    let config = Config {
        rpc_url,
        rpc_timeout: Duration::from_secs(2),
        collection_interval: Duration::from_secs(5),
        output_file: output_file.clone(),
    };

    let client = GethClient::new(&config).unwrap();

    let mut scheduler =
        MonitoringScheduler::new(&client, &config, "test-node-no-admin")
            .unwrap();

    let observation = scheduler.collect_once().await.unwrap();

    assert_eq!(observation.blockchain.block_number, Some(436));
    assert_eq!(observation.p2p.peer_count, Some(10));
    assert_eq!(observation.p2p.network_listening, Some(true));

    assert_eq!(observation.p2p.admin_peer_count, None);
    assert_eq!(observation.p2p.peer_details_available, Some(false));

    assert_eq!(observation.rpc.rpc_available, Some(true));

    let content = std::fs::read_to_string(&output_file).unwrap();
    let saved: Value = serde_json::from_str(content.trim()).unwrap();

    assert_eq!(saved["peer_count"], 10);
    assert_eq!(saved["peer_details_available"], false);

    let _ = std::fs::remove_file(output_file);
}

// Test 4: Detect complete peer rotation (50 A peers -> 50 B peers).
#[tokio::test]
async fn scheduler_detects_complete_peer_rotation() {
    let rpc_url = spawn_fake_geth_peer_rotation().await;

    let output_file = std::env::temp_dir().join(format!(
        "eva_guard_peer_rotation_{}.jsonl",
        std::process::id()
    ));

    let _ = std::fs::remove_file(&output_file);

    let config = Config {
        rpc_url,
        rpc_timeout: Duration::from_secs(2),
        collection_interval: Duration::from_secs(5),
        output_file: output_file.clone(),
    };

    let client = GethClient::new(&config).unwrap();

    let mut scheduler =
        MonitoringScheduler::new(&client, &config, "peer-rotation-test")
            .unwrap();

    // Cycle 1: peers A1 to A50.
    let first = scheduler.collect_once().await.unwrap();

    assert_eq!(first.p2p.peer_count, Some(50));
    assert_eq!(first.p2p.admin_peer_count, Some(50));

    // The first cycle has no previous peer list.
    assert_eq!(first.p2p.new_peers, None);
    assert_eq!(first.p2p.disconnected_peers, None);
    assert_eq!(first.p2p.peer_churn_count, None);

    // Cycle 2: peers B1 to B50.
    let second = scheduler.collect_once().await.unwrap();

    assert_eq!(second.p2p.peer_count, Some(50));
    assert_eq!(second.p2p.admin_peer_count, Some(50));

    // All 50 B peers are new.
    assert_eq!(second.p2p.new_peers, Some(50));

    // All 50 A peers have disconnected.
    assert_eq!(second.p2p.disconnected_peers, Some(50));

    // Total peer changes = 50 + 50.
    assert_eq!(second.p2p.peer_churn_count, Some(100));

    // Verify that both observations were persisted.
    let content = std::fs::read_to_string(&output_file).unwrap();
    let lines: Vec<&str> = content.lines().collect();

    assert_eq!(lines.len(), 2);

    let first_saved: Value = serde_json::from_str(lines[0]).unwrap();
    let second_saved: Value = serde_json::from_str(lines[1]).unwrap();

    assert_eq!(first_saved["admin_peer_count"], 50);
    assert!(first_saved["new_peers"].is_null());

    assert_eq!(second_saved["new_peers"], 50);
    assert_eq!(second_saved["disconnected_peers"], 50);
    assert_eq!(second_saved["peer_churn_count"], 100);

    let _ = std::fs::remove_file(output_file);
}


#[tokio::test]
async fn scheduler_handles_empty_peer_list() {
    let (rpc_url, _server) = spawn_fake_geth_empty_peers().await;

    let temp_dir = tempfile::tempdir().unwrap();
    let output_file = temp_dir.path().join("observations.jsonl");

    let config = Config {
        rpc_url,
        output_file: output_file.clone(),
        ..Config::default()
    };

    let client = GethClient::new(&config).unwrap();

    let mut scheduler =
        MonitoringScheduler::new(&client, &config, "test-node".to_string())
            .unwrap();

    let observation = scheduler.collect_once().await.unwrap();

    assert_eq!(observation.p2p.peer_count, Some(0));
    assert_eq!(observation.p2p.admin_peer_count, Some(0));
    assert_eq!(observation.p2p.peer_details_available, Some(true));

    assert_eq!(observation.p2p.new_peers, None);
    assert_eq!(observation.p2p.disconnected_peers, None);
    assert_eq!(observation.p2p.peer_churn_count, None);

    assert_eq!(observation.rpc.rpc_available, Some(true));

    assert!(output_file.exists());

    println!("Empty peer list test passed");
}