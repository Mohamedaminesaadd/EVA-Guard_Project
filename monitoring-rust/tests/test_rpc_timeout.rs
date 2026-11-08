
use monitoring_rust::config::Config;
use monitoring_rust::geth::client::GethClient;
use monitoring_rust::monitoring::scheduler::MonitoringScheduler;

use std::time::{Duration, Instant};
use tokio::net::TcpListener;

#[tokio::test]
async fn scheduler_handles_rpc_timeout() {
    // 1. Create a fake Geth server.
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap();

    let address = listener.local_addr().unwrap();
    let rpc_url = format!("http://{}", address);

    // Keep connections open without sending HTTP responses.
    let server = tokio::spawn(async move {
        loop {
            let (socket, _) = listener.accept().await.unwrap();

            tokio::spawn(async move {
                let _socket = socket;

                // Hold the connection longer than the RPC timeout.
                tokio::time::sleep(Duration::from_secs(10)).await;
            });
        }
    });

    // 2. Configure a short timeout.
    let output_file = std::env::temp_dir().join(format!(
        "eva_guard_rpc_timeout_{}.jsonl",
        std::process::id()
    ));

    let _ = std::fs::remove_file(&output_file);

    let config = Config {
        rpc_url,
        rpc_timeout: Duration::from_millis(500),
        output_file: output_file.clone(),
        ..Config::default()
    };

    // 3. Initialize client and scheduler.
    let client = GethClient::new(&config).unwrap();

    let mut scheduler = MonitoringScheduler::new(
        &client,
        &config,
        "test-node".to_string(),
    )
    .unwrap();

    // 4. Collect observation.
    let start = Instant::now();

    let observation = scheduler.collect_once().await.unwrap();

    let elapsed = start.elapsed();

    // 5. Verify RPC timeout.
    assert_eq!(
        observation.rpc.rpc_available,
        Some(false),
        "RPC should be unavailable after timeout"
    );

    assert!(
        elapsed >= Duration::from_millis(400),
        "Timeout happened too early: {:?}",
        elapsed
    );

    // 6. Verify JSONL persistence.
    assert!(output_file.exists());

    let content = std::fs::read_to_string(&output_file).unwrap();

    assert!(!content.trim().is_empty());

    println!("RPC timeout handled in {:?}", elapsed);
    println!("Observation persisted successfully");

    // 7. Cleanup.
    server.abort();

    let _ = std::fs::remove_file(&output_file);
}
