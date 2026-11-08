use std::fs;
use std::io;
use std::time::Instant;

use crate::models::observation::NetworkMetrics;

/// Previous network counters used to calculate traffic rates.
struct NetworkSample {
    rx_bytes: u64,
    tx_bytes: u64,
    timestamp: Instant,
}

/// Collects host-wide network metrics from Linux /proc.
pub struct NetworkCollector {
    previous_sample: Option<NetworkSample>,
}

impl NetworkCollector {
    /// Creates a new network collector.
    pub fn new() -> Self {
        Self {
            previous_sample: None,
        }
    }

    /// Collects network counters, traffic rates and TCP connections.
    pub fn collect(&mut self) -> io::Result<NetworkMetrics> {
        let (rx_bytes, tx_bytes) = read_network_counters()?;
        let tcp_connections = read_tcp_established_connections()?;

        let now = Instant::now();

        let (rx_rate, tx_rate) = if let Some(previous) = &self.previous_sample {
            let elapsed = now.duration_since(previous.timestamp).as_secs_f64();

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

/// Reads total RX/TX bytes from /proc/net/dev.
/// The loopback interface (lo) is excluded.
fn read_network_counters() -> io::Result<(u64, u64)> {
    let content = fs::read_to_string("/proc/net/dev")?;

    let mut total_rx = 0u64;
    let mut total_tx = 0u64;

    for line in content.lines().skip(2) {
        let Some((interface, statistics)) = line.split_once(':') else {
            continue;
        };

        if interface.trim() == "lo" {
            continue;
        }

        let values: Vec<u64> = statistics
            .split_whitespace()
            .filter_map(|value| value.parse::<u64>().ok())
            .collect();

        if values.len() < 9 {
            continue;
        }

        // First counter: received bytes.
        total_rx = total_rx.saturating_add(values[0]);

        // Ninth counter: transmitted bytes.
        total_tx = total_tx.saturating_add(values[8]);
    }

    Ok((total_rx, total_tx))
}

/// Counts system-wide established TCP connections.
/// Reads both IPv4 and IPv6 connection tables.
fn read_tcp_established_connections() -> io::Result<u64> {
    let mut total = 0u64;

    for path in ["/proc/net/tcp", "/proc/net/tcp6"] {
        let content = fs::read_to_string(path)?;

        for line in content.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();

            // TCP state is the fourth column.
            if fields.get(3) == Some(&"01") {
                total += 1;
            }
        }
    }

    Ok(total)
}
