
use std::io;
use std::process::Command;
use std::time::Instant;
use std::path::Path;

use sysinfo::Networks;

use crate::models::observation::NetworkMetrics;

/// Previous network counters used to calculate traffic rates.
struct NetworkSample {
    rx_bytes: u64,
    tx_bytes: u64,
    timestamp: Instant,
}

/// Collects host-wide network metrics on Windows.
pub struct NetworkCollector {
    previous_sample: Option<NetworkSample>,
    networks: Networks,
}

impl NetworkCollector {
    /// Creates a new network collector.
    pub fn new() -> Self {
        Self {
            previous_sample: None,
            networks: Networks::new_with_refreshed_list(),
        }
    }

    /// Collects network counters, traffic rates and TCP connections.
    pub fn collect(&mut self) -> io::Result<NetworkMetrics> {

        // Read Windows network counters.
        let (rx_bytes, tx_bytes) =
            read_network_counters(&mut self.networks);

        // Count established TCP connections.
        let tcp_connections =
            read_tcp_established_connections()?;

        let now = Instant::now();

        // Calculate traffic rates.
        let (rx_rate, tx_rate) =
            if let Some(previous) = &self.previous_sample {

                let elapsed = now
                    .duration_since(previous.timestamp)
                    .as_secs_f64();

                if elapsed > 0.0 {
                    let rx = rx_bytes
                        .checked_sub(previous.rx_bytes)
                        .map(|delta| delta as f64 / elapsed);

                    let tx = tx_bytes
                        .checked_sub(previous.tx_bytes)
                        .map(|delta| delta as f64 / elapsed);

                    (rx, tx)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

        // Save current sample.
        self.previous_sample = Some(NetworkSample {
            rx_bytes,
            tx_bytes,
            timestamp: now,
        });

        Ok(NetworkMetrics {
            network_rx_bytes_total: Some(rx_bytes),
            network_tx_bytes_total: Some(tx_bytes),
            network_rx_bytes_per_sec: rx_rate,
            network_tx_bytes_per_sec: tx_rate,
            tcp_established_connections: Some(tcp_connections),
        })
    }
}

impl Default for NetworkCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Reads total received and transmitted bytes on Windows.
fn read_network_counters(
    networks: &mut Networks,
) -> (u64, u64) {

    // Refresh network interfaces.
    networks.refresh(true);

    let mut total_rx = 0u64;
    let mut total_tx = 0u64;

    for (interface, data) in networks.iter() {

        let name = interface.to_lowercase();

        // Exclude loopback interfaces.
        if name.contains("loopback") {
            continue;
        }

        // Total received bytes.
        total_rx = total_rx.saturating_add(
            data.total_received()
        );

        // Total transmitted bytes.
        total_tx = total_tx.saturating_add(
            data.total_transmitted()
        );
    }

    (total_rx, total_tx)
}

/// Counts established TCP connections on Windows.
fn read_tcp_established_connections() -> io::Result<u64> {

    // Absolute PowerShell path to avoid PATH errors.
    let system_root = std::env::var("SystemRoot")
        .unwrap_or_else(|_| String::from(r"C:\Windows"));

    let powershell = Path::new(&system_root)
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");

    let output = Command::new(&powershell)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "@(Get-NetTCPConnection -State Established -ErrorAction Stop).Count",
        ])
        .output()
        .map_err(|e| {
            io::Error::new(
                e.kind(),
                format!(
                    "Cannot launch PowerShell at {}: {}",
                    powershell.display(),
                    e
                ),
            )
        })?;

    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "PowerShell TCP query failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        ));
    }

    let result = String::from_utf8_lossy(&output.stdout);

    let count = result.trim().parse::<u64>().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid TCP connection count: {}", e),
        )
    })?;

    Ok(count)
}
