
use std::error::Error;

use monitoring_rust::config::Config;
use monitoring_rust::geth::client::GethClient;
use monitoring_rust::monitoring::scheduler::MonitoringScheduler;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("========================================");
    println!("     EVA-Guard Monitoring Engine");
    println!("========================================");

    // 1. Load configuration
    let config = Config::default();

    println!("\n[1] Configuration loaded");
    println!("RPC URL: {}", config.rpc_url);
    println!("Collection interval: {:?}", config.collection_interval);
    println!("Output file: {:?}", config.output_file);

    // 2. Initialize Geth client
    let client = GethClient::new(&config)?;

    println!("\n[2] Geth client initialized");

    // 3. Initialize monitoring scheduler
    let mut scheduler = MonitoringScheduler::new(
        &client,
        &config,
        "eva-guard-node-01",
    )?;

    println!("[3] Monitoring scheduler initialized");

    // 4. Start continuous monitoring
    println!("\n========================================");
    println!("       CONTINUOUS MONITORING");
    println!("========================================");

    println!("Monitoring started...");
    println!("Press Ctrl+C to stop.\n");

    scheduler.run().await?;

    Ok(())
}
