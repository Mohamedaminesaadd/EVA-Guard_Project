
use monitoring_rust::config::Config;
use monitoring_rust::geth::client::GethClient;
use monitoring_rust::monitoring::scheduler::MonitoringScheduler;

use serde_json::{json, Value};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

// Read a complete HTTP request using Content-Length.
async fn read_request(socket: &mut TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];

    let header_end;

    loop {
        let n = socket.read(&mut chunk).await.unwrap();
        assert!(n > 0, "Connection closed before HTTP headers");

        buffer.extend_from_slice(&chunk[..n]);

        if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = pos + 4;
            break;
        }
    }

    let headers = String::from_utf8_lossy(&buffer[..header_end]);

    let content_length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;

            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .expect("Missing Content-Length header");

    while buffer.len() < header_end + content_length {
        let n = socket.read(&mut chunk).await.unwrap();
        assert!(n > 0, "Connection closed before HTTP body completed");

        buffer.extend_from_slice(&chunk[..n]);
    }

    String::from_utf8(buffer[header_end..header_end + content_length].to_vec())
        .unwrap()
}

// Start a fake Geth server with an empty peer list.
async fn spawn_fake_geth_empty_peers(
) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap();

    let address = listener.local_addr().unwrap();
    let rpc_url = format!("http://{}", address);

    let server = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();

            tokio::spawn(async move {
                let body = read_request(&mut socket).await;

                let rpc_request: Value =
                    serde_json::from_str(&body).unwrap();

                let method = rpc_request["method"]
                    .as_str()
                    .unwrap();

                let id = rpc_request["id"].clone();

                let result = match method {
                    "eth_blockNumber" => json!("0x1b4"),
                    "eth_syncing" => json!(false),
                    "net_peerCount" => json!("0x0"),
                    "net_listening" => json!(true),
                    "admin_peers" => json!([]),
                    _ => {
                        json!({
                            "code": -32601,
                            "message": "Method not found"
                        })
                    }
                };

                let response = if method == "eth_blockNumber"
                    || method == "eth_syncing"
                    || method == "net_peerCount"
                    || method == "net_listening"
                    || method == "admin_peers"
                {
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": result
                    })
                } else {
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": result
                    })
                };

                let response_body = response.to_string();

                let http_response = format!(
                    "HTTP/1.1 200 OK\r\n\
                     Content-Type: application/json\r\n\
                     Content-Length: {}\r\n\
                     Connection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );

                socket
                    .write_all(http_response.as_bytes())
                    .await
                    .unwrap();
            });
        }
    });

    (rpc_url, server)
}

// Integration test: Geth is available but has no connected peers.
#[tokio::test]
async fn scheduler_handles_empty_peer_list() {
    let (rpc_url, server) = spawn_fake_geth_empty_peers().await;

    let output_file = std::env::temp_dir().join(format!(
        "eva_guard_empty_peers_{}.jsonl",
        std::process::id()
    ));

    let _ = std::fs::remove_file(&output_file);

    let config = Config {
        rpc_url,
        output_file: output_file.clone(),
        rpc_timeout: Duration::from_secs(3),
        ..Config::default()
    };

    let client = GethClient::new(&config).unwrap();

    let mut scheduler = MonitoringScheduler::new(
        &client,
        &config,
        "test-node".to_string(),
    )
    .unwrap();

    let observation = scheduler.collect_once().await.unwrap();

    // Geth and RPC are available.
    assert_eq!(observation.rpc.rpc_available, Some(true));

    // Geth has zero connected peers.
    assert_eq!(observation.p2p.peer_count, Some(0));
    assert_eq!(observation.p2p.admin_peer_count, Some(0));

    // The admin namespace is available and returned an empty list.
    assert_eq!(
        observation.p2p.peer_details_available,
        Some(true)
    );

    // First collection: no previous sample for comparison.
    assert_eq!(observation.p2p.new_peers, None);
    assert_eq!(observation.p2p.disconnected_peers, None);
    assert_eq!(observation.p2p.peer_churn_count, None);

    // Blockchain and network listening are still available.
    assert_eq!(observation.blockchain.block_number, Some(436));
    assert_eq!(observation.p2p.network_listening, Some(true));

    // Verify JSONL persistence.
    assert!(output_file.exists());

    let content = std::fs::read_to_string(&output_file).unwrap();
    let saved: Value = serde_json::from_str(content.trim()).unwrap();

    assert_eq!(saved["peer_count"], json!(0));
    assert_eq!(saved["admin_peer_count"], json!(0));
    assert_eq!(saved["rpc_available"], json!(true));

    println!("Empty peer list integration test passed");

    // Cleanup.
    drop(scheduler);
    drop(client);
    server.abort();
    let _ = std::fs::remove_file(&output_file);
}
