//! Integration tests for the Geth JSON-RPC client.
//!
//! No real Geth node is needed: a tiny fake HTTP server runs on a local TcpListener.

use std::sync::Arc;
use std::time::Duration;

use monitoring_rust::config::Config;
use monitoring_rust::geth::client::{GethClient, GethError, SyncStatus};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

// ---------------------------------------------------------------------------
// Fake server helpers
// ---------------------------------------------------------------------------

/// Read one HTTP request and return its body.
async fn read_request(socket: &mut TcpStream) -> String {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        match socket.read(&mut chunk).await {
            Ok(0) | Err(_) => return String::new(),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&buf[..pos]).to_lowercase();
            let content_length = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            let body_start = pos + 4;
            if buf.len() >= body_start + content_length {
                return String::from_utf8_lossy(&buf[body_start..body_start + content_length])
                    .into_owned();
            }
        }
    }
}

/// Start a fake HTTP server. `handler` maps the request body to `(status, response body)`;
/// `delay` is applied before answering (used to trigger timeouts).
async fn spawn_server<F>(delay: Duration, handler: F) -> String
where
    F: Fn(&str) -> (u16, String) + Send + Sync + 'static,
{
    let handler = Arc::new(handler);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let handler = Arc::clone(&handler);
            tokio::spawn(async move {
                let request_body = read_request(&mut socket).await;
                tokio::time::sleep(delay).await;
                let (status, body) = handler(&request_body);
                let response = format!(
                    "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });

    format!("http://{addr}")
}

fn rpc_result(request: &Value, result: Value) -> (u16, String) {
    let body = json!({ "jsonrpc": "2.0", "id": request["id"], "result": result });
    (200, body.to_string())
}

fn rpc_error(request: &Value, code: i64, message: &str) -> (u16, String) {
    let body = json!({
        "jsonrpc": "2.0",
        "id": request["id"],
        "error": { "code": code, "message": message }
    });
    (200, body.to_string())
}

/// Behaves like a Geth node with the `admin` namespace enabled.
fn fake_geth(body: &str) -> (u16, String) {
    let request: Value = serde_json::from_str(body).expect("client must send valid JSON");
    assert_eq!(request["jsonrpc"], "2.0");
    match request["method"].as_str().unwrap() {
        "eth_blockNumber" => rpc_result(&request, json!("0x1b4")),
        "net_peerCount" => rpc_result(&request, json!("0xa")),
        "net_listening" => rpc_result(&request, json!(true)),
        "eth_syncing" => rpc_result(&request, json!(false)),
        "admin_peers" => rpc_result(
            &request,
            json!([
                { "id": "aaaa", "name": "Geth/v1", "network": { "remoteAddress": "1.2.3.4:30303", "inbound": false } },
                { "id": "bbbb" }
            ]),
        ),
        other => rpc_error(
            &request,
            -32601,
            &format!("the method {other} does not exist/is not available"),
        ),
    }
}

fn client_for(url: String, timeout: Duration) -> GethClient {
    let config = Config {
        rpc_url: url,
        rpc_timeout: timeout,
        ..Config::default()
    };
    GethClient::new(&config).unwrap()
}

const TIMEOUT: Duration = Duration::from_secs(2);

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn reads_block_number_and_peer_count() {
    let client = client_for(spawn_server(Duration::ZERO, fake_geth).await, TIMEOUT);

    let block = client.block_number().await.unwrap();
    assert_eq!(block.value, 436);
    assert!(block.latency > Duration::ZERO && block.latency < TIMEOUT);

    let peers = client.peer_count().await.unwrap();
    assert_eq!(peers.value, 10);
}

#[tokio::test]
async fn reads_listening_syncing_and_admin_peers() {
    let client = client_for(spawn_server(Duration::ZERO, fake_geth).await, TIMEOUT);

    assert!(client.listening().await.unwrap().value);
    assert_eq!(
        client.syncing().await.unwrap().value,
        SyncStatus::NotSyncing
    );

    let peers = client.admin_peers().await.unwrap().value;
    assert_eq!(peers.len(), 2);
    assert_eq!(peers[0].id, "aaaa");
    assert_eq!(peers[0].network.as_ref().unwrap().inbound, Some(false));
    assert_eq!(peers[1].id, "bbbb");
}

#[tokio::test]
async fn disabled_admin_namespace_is_reported_as_unavailable() {
    let url = spawn_server(Duration::ZERO, |body| {
        let request: Value = serde_json::from_str(body).unwrap();
        rpc_error(
            &request,
            -32601,
            "the method admin_peers does not exist/is not available",
        )
    })
    .await;
    let client = client_for(url, TIMEOUT);

    let err = client.admin_peers().await.unwrap_err();
    assert!(err.is_method_unavailable(), "got {err:?}");
}

#[tokio::test]
async fn other_rpc_errors_are_not_reported_as_unavailable() {
    let url = spawn_server(Duration::ZERO, |body| {
        let request: Value = serde_json::from_str(body).unwrap();
        rpc_error(&request, -32000, "internal failure")
    })
    .await;
    let client = client_for(url, TIMEOUT);

    let err = client.block_number().await.unwrap_err();
    assert!(
        matches!(&err, GethError::Rpc { code: -32000, .. }),
        "got {err:?}"
    );
    assert!(!err.is_method_unavailable());
}

#[tokio::test]
async fn http_error_status_is_reported() {
    let url = spawn_server(Duration::ZERO, |_| (500, "oops".to_string())).await;
    let client = client_for(url, TIMEOUT);

    let err = client.block_number().await.unwrap_err();
    assert!(
        matches!(&err, GethError::HttpStatus(status) if status.as_u16() == 500),
        "got {err:?}"
    );
}

#[tokio::test]
async fn malformed_json_is_an_invalid_response() {
    let url = spawn_server(Duration::ZERO, |_| (200, "not json".to_string())).await;
    let client = client_for(url, TIMEOUT);

    let err = client.block_number().await.unwrap_err();
    assert!(matches!(err, GethError::InvalidResponse(_)), "got {err:?}");
}

#[tokio::test]
async fn missing_result_is_an_invalid_response() {
    let url = spawn_server(Duration::ZERO, |_| {
        (200, json!({ "jsonrpc": "2.0", "id": 1 }).to_string())
    })
    .await;
    let client = client_for(url, TIMEOUT);

    let err = client.block_number().await.unwrap_err();
    assert!(matches!(err, GethError::InvalidResponse(_)), "got {err:?}");
}

#[tokio::test]
async fn mismatched_response_id_is_an_invalid_response() {
    let url = spawn_server(Duration::ZERO, |_| {
        (
            200,
            json!({ "jsonrpc": "2.0", "id": 99, "result": "0x1" }).to_string(),
        )
    })
    .await;
    let client = client_for(url, TIMEOUT);

    let err = client.block_number().await.unwrap_err();
    assert!(matches!(err, GethError::InvalidResponse(_)), "got {err:?}");
}

#[tokio::test]
async fn invalid_quantity_is_an_invalid_response() {
    let url = spawn_server(Duration::ZERO, |body| {
        let request: Value = serde_json::from_str(body).unwrap();
        rpc_result(&request, json!("not-hex"))
    })
    .await;
    let client = client_for(url, TIMEOUT);

    let err = client.peer_count().await.unwrap_err();
    assert!(matches!(err, GethError::InvalidResponse(_)), "got {err:?}");
}

#[tokio::test]
async fn slow_node_triggers_a_timeout() {
    let url = spawn_server(Duration::from_secs(2), fake_geth).await;
    let client = client_for(url, Duration::from_millis(200));

    let err = client.block_number().await.unwrap_err();
    assert!(matches!(err, GethError::Timeout(_)), "got {err:?}");
}

#[tokio::test]
async fn closed_port_is_a_connection_error() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener); // nothing listens on this port any more

    let client = client_for(format!("http://{addr}"), TIMEOUT);

    let err = client.block_number().await.unwrap_err();
    assert!(matches!(err, GethError::Connection(_)), "got {err:?}");
}
