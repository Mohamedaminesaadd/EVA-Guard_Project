
use std::io;

use sysinfo::System;

use crate::models::observation::SystemMetrics;

/// Collects system-level metrics on Windows 10.
pub struct SystemCollector {
    system: System,
    cpu_initialized: bool,
}

impl SystemCollector {
    /// Creates a new system collector.
    pub fn new() -> Self {
        let mut system = System::new();

        // Initialize CPU counters.
        system.refresh_cpu_usage();

        Self {
            system,
            cpu_initialized: false,
        }
    }

    /// Collect CPU and memory metrics.
    ///
    /// The first CPU measurement returns None because
    /// CPU usage requires two samples.
    pub fn collect(&mut self) -> io::Result<SystemMetrics> {

        // Refresh CPU information.
        self.system.refresh_cpu_usage();

        // Refresh RAM information.
        self.system.refresh_memory();

        // Calculate CPU usage percentage.
        let cpu_usage_percent = if self.cpu_initialized {
            Some(self.system.global_cpu_usage() as f64)
        } else {
            self.cpu_initialized = true;
            None
        };

        // sysinfo returns memory values in bytes.
        // Convert bytes to MiB.
        let memory_total_mb =
            self.system.total_memory() / (1024 * 1024);

        let memory_used_mb =
            self.system.used_memory() / (1024 * 1024);

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
