use std::fs;
use std::io;

use crate::models::observation::SystemMetrics;

/// CPU counters read from /proc/stat.
#[derive(Debug, Clone, Copy)]
struct CpuSample {
    total: u64,
    idle: u64,
}

/// Collects system-level metrics from Linux /proc.
pub struct SystemCollector {
    previous_cpu_sample: Option<CpuSample>,
}

impl SystemCollector {
    pub fn new() -> Self {
        Self {
            previous_cpu_sample: None,
        }
    }

    /// Collect CPU and memory metrics.
    ///
    /// The first CPU measurement returns None because CPU usage
    /// requires two samples to calculate a delta.
    pub fn collect(&mut self) -> io::Result<SystemMetrics> {
        let cpu_content = fs::read_to_string("/proc/stat")?;
        let mem_content = fs::read_to_string("/proc/meminfo")?;

        let current_cpu = parse_cpu_sample(&cpu_content)?;

        let cpu_usage_percent = calculate_cpu_usage(self.previous_cpu_sample, current_cpu);

        self.previous_cpu_sample = Some(current_cpu);

        let (memory_used_mb, memory_total_mb) = parse_memory_info(&mem_content)?;

        Ok(SystemMetrics {
            cpu_usage_percent,
            memory_used_mb: Some(memory_used_mb),
            memory_total_mb: Some(memory_total_mb),
        })
    }
}

impl Default for SystemCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse the aggregate CPU line from /proc/stat.
fn parse_cpu_sample(content: &str) -> io::Result<CpuSample> {
    let cpu_line = content
        .lines()
        .find(|line| line.starts_with("cpu "))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "CPU statistics not found in /proc/stat",
            )
        })?;

    let values: Vec<u64> = cpu_line
        .split_whitespace()
        .skip(1)
        .map(|value| {
            value.parse::<u64>().map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid CPU counter in /proc/stat",
                )
            })
        })
        .collect::<Result<_, _>>()?;

    if values.len() < 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Insufficient CPU counters in /proc/stat",
        ));
    }

    // CPU counters:
    // user, nice, system, idle, iowait, irq, softirq, steal.
    // guest and guest_nice are excluded because they are already
    // included in user and nice.
    let total: u64 = values.iter().take(8).copied().sum();

    let idle = values[3].saturating_add(values.get(4).copied().unwrap_or(0));

    Ok(CpuSample { total, idle })
}

/// Calculate CPU usage between two samples.
fn calculate_cpu_usage(previous: Option<CpuSample>, current: CpuSample) -> Option<f64> {
    let previous = previous?;

    let total_delta = current.total.checked_sub(previous.total)?;
    let idle_delta = current.idle.checked_sub(previous.idle)?;

    if total_delta == 0 || idle_delta > total_delta {
        return None;
    }

    let usage = (total_delta - idle_delta) as f64 / total_delta as f64 * 100.0;

    Some(usage.clamp(0.0, 100.0))
}

/// Parse memory information from /proc/meminfo.
///
/// Values are expressed in KiB.
/// Convert them to MiB by dividing by 1024.
fn parse_memory_info(content: &str) -> io::Result<(u64, u64)> {
    let mut total_kb: Option<u64> = None;
    let mut available_kb: Option<u64> = None;

    for line in content.lines() {
        let mut parts = line.split_whitespace();

        match parts.next() {
            Some("MemTotal:") => {
                total_kb = parts.next().and_then(|v| v.parse().ok());
            }
            Some("MemAvailable:") => {
                available_kb = parts.next().and_then(|v| v.parse().ok());
            }
            _ => {}
        }
    }

    let total_kb = total_kb.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "MemTotal not found in /proc/meminfo",
        )
    })?;

    let available_kb = available_kb.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "MemAvailable not found in /proc/meminfo",
        )
    })?;

    let used_kb = total_kb.saturating_sub(available_kb);

    let memory_used_mb = used_kb / 1024;
    let memory_total_mb = total_kb / 1024;

    Ok((memory_used_mb, memory_total_mb))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cpu_sample() {
        let content = "\
cpu 100 20 30 400 10 5 5 2 0 0
cpu0 50 10 15 200 5 2 2 1 0 0";

        let sample = parse_cpu_sample(content).unwrap();

        assert_eq!(sample.total, 572);
        assert_eq!(sample.idle, 410);
    }

    #[test]
    fn test_calculate_cpu_usage() {
        let previous = CpuSample {
            total: 1000,
            idle: 800,
        };

        let current = CpuSample {
            total: 1200,
            idle: 900,
        };

        let usage = calculate_cpu_usage(Some(previous), current).unwrap();

        assert!((usage - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_first_cpu_sample_returns_none() {
        let current = CpuSample {
            total: 1000,
            idle: 800,
        };

        assert_eq!(calculate_cpu_usage(None, current), None);
    }

    #[test]
    fn test_parse_memory_info() {
        let content = "\
MemTotal:       16384000 kB
MemFree:         2000000 kB
MemAvailable:    6000000 kB
Buffers:          100000 kB
";

        let (used, total) = parse_memory_info(content).unwrap();

        assert_eq!(used, 10140);
        assert_eq!(total, 16000);
    }
}
