//! Shared monitoring configuration (Phases 2-3).
//!
//! Values come from environment variables. A missing variable falls back to a
//! default; a variable that is set but invalid is a hard error.

use std::env;
use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

pub const ENV_RPC_URL: &str = "RPC_URL";
pub const ENV_RPC_TIMEOUT: &str = "RPC_TIMEOUT";
pub const ENV_COLLECTION_INTERVAL: &str = "COLLECTION_INTERVAL";
pub const ENV_OUTPUT_FILE: &str = "OUTPUT_FILE";

pub const DEFAULT_RPC_URL: &str = "http://127.0.0.1:8545";
pub const DEFAULT_RPC_TIMEOUT_SECS: u64 = 3;
pub const DEFAULT_COLLECTION_INTERVAL_SECS: u64 = 5;
pub const DEFAULT_OUTPUT_FILE: &str = "data/observations.jsonl";

/// Errors raised while loading or validating the configuration.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("{var} is set but empty")]
    Empty { var: &'static str },

    #[error("RPC_URL must start with http:// or https:// and include a host (got `{0}`)")]
    InvalidRpcUrl(String),

    #[error("{var} must be a positive integer number of seconds (got `{value}`)")]
    InvalidSeconds { var: &'static str, value: String },
}

/// Validated monitoring configuration shared by every module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Geth JSON-RPC HTTP endpoint.
    pub rpc_url: String,
    /// Maximum duration of one JSON-RPC request (connection + response).
    pub rpc_timeout: Duration,
    /// Delay between two collection cycles.
    pub collection_interval: Duration,
    /// JSONL file where observations are appended.
    pub output_file: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rpc_url: DEFAULT_RPC_URL.to_string(),
            rpc_timeout: Duration::from_secs(DEFAULT_RPC_TIMEOUT_SECS),
            collection_interval: Duration::from_secs(DEFAULT_COLLECTION_INTERVAL_SECS),
            output_file: PathBuf::from(DEFAULT_OUTPUT_FILE),
        }
    }
}

impl Config {
    /// Load the configuration from the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|key| env::var_os(key).map(|v| v.to_string_lossy().into_owned()))
    }

    /// Load the configuration from an arbitrary key lookup (used by tests).
    pub fn from_lookup<F>(lookup: F) -> Result<Self, ConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        Ok(Self {
            rpc_url: parse_rpc_url(lookup(ENV_RPC_URL))?,
            rpc_timeout: parse_seconds(
                lookup(ENV_RPC_TIMEOUT),
                ENV_RPC_TIMEOUT,
                DEFAULT_RPC_TIMEOUT_SECS,
            )?,
            collection_interval: parse_seconds(
                lookup(ENV_COLLECTION_INTERVAL),
                ENV_COLLECTION_INTERVAL,
                DEFAULT_COLLECTION_INTERVAL_SECS,
            )?,
            output_file: parse_output_file(lookup(ENV_OUTPUT_FILE))?,
        })
    }
}

fn parse_rpc_url(raw: Option<String>) -> Result<String, ConfigError> {
    let Some(raw) = raw else {
        return Ok(DEFAULT_RPC_URL.to_string());
    };
    let value = raw.trim().to_string();
    if value.is_empty() {
        return Err(ConfigError::Empty { var: ENV_RPC_URL });
    }
    let has_host = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
        .is_some_and(|rest| !rest.is_empty());
    if has_host {
        Ok(value)
    } else {
        Err(ConfigError::InvalidRpcUrl(value))
    }
}

/// Parse a strictly positive number of seconds (shared by the timeout and the interval).
fn parse_seconds(
    raw: Option<String>,
    var: &'static str,
    default_secs: u64,
) -> Result<Duration, ConfigError> {
    let Some(raw) = raw else {
        return Ok(Duration::from_secs(default_secs));
    };
    let value = raw.trim();
    if value.is_empty() {
        return Err(ConfigError::Empty { var });
    }
    match value.parse::<u64>() {
        Ok(secs) if secs > 0 => Ok(Duration::from_secs(secs)),
        _ => Err(ConfigError::InvalidSeconds {
            var,
            value: value.to_string(),
        }),
    }
}

fn parse_output_file(raw: Option<String>) -> Result<PathBuf, ConfigError> {
    let Some(raw) = raw else {
        return Ok(PathBuf::from(DEFAULT_OUTPUT_FILE));
    };
    let value = raw.trim();
    if value.is_empty() {
        return Err(ConfigError::Empty {
            var: ENV_OUTPUT_FILE,
        });
    }
    Ok(PathBuf::from(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |key| map.get(key).cloned()
    }

    fn invalid_seconds(var: &'static str, value: &str) -> ConfigError {
        ConfigError::InvalidSeconds {
            var,
            value: value.to_string(),
        }
    }

    #[test]
    fn defaults_when_nothing_is_set() {
        let config = Config::from_lookup(lookup(&[])).unwrap();
        assert_eq!(config, Config::default());
        assert_eq!(config.rpc_url, "http://127.0.0.1:8545");
        assert_eq!(config.rpc_timeout, Duration::from_secs(3));
        assert_eq!(config.collection_interval, Duration::from_secs(5));
        assert_eq!(config.output_file, PathBuf::from("data/observations.jsonl"));
    }

    #[test]
    fn custom_values_are_applied() {
        let config = Config::from_lookup(lookup(&[
            ("RPC_URL", "https://node.local:9545"),
            ("RPC_TIMEOUT", "2"),
            ("COLLECTION_INTERVAL", "10"),
            ("OUTPUT_FILE", "/tmp/obs.jsonl"),
        ]))
        .unwrap();
        assert_eq!(config.rpc_url, "https://node.local:9545");
        assert_eq!(config.rpc_timeout, Duration::from_secs(2));
        assert_eq!(config.collection_interval, Duration::from_secs(10));
        assert_eq!(config.output_file, PathBuf::from("/tmp/obs.jsonl"));
    }

    #[test]
    fn values_are_trimmed() {
        let config = Config::from_lookup(lookup(&[
            ("RPC_URL", "  http://127.0.0.1:8545  "),
            ("COLLECTION_INTERVAL", " 7 "),
        ]))
        .unwrap();
        assert_eq!(config.rpc_url, "http://127.0.0.1:8545");
        assert_eq!(config.collection_interval, Duration::from_secs(7));
    }

    #[test]
    fn empty_rpc_url_is_rejected() {
        let result = Config::from_lookup(lookup(&[("RPC_URL", "   ")]));
        assert_eq!(result, Err(ConfigError::Empty { var: "RPC_URL" }));
    }

    #[test]
    fn rpc_url_without_scheme_is_rejected() {
        let result = Config::from_lookup(lookup(&[("RPC_URL", "127.0.0.1:8545")]));
        assert_eq!(
            result,
            Err(ConfigError::InvalidRpcUrl("127.0.0.1:8545".to_string()))
        );
    }

    #[test]
    fn rpc_url_with_scheme_only_is_rejected() {
        let result = Config::from_lookup(lookup(&[("RPC_URL", "http://")]));
        assert_eq!(
            result,
            Err(ConfigError::InvalidRpcUrl("http://".to_string()))
        );
    }

    #[test]
    fn zero_interval_is_rejected() {
        let result = Config::from_lookup(lookup(&[("COLLECTION_INTERVAL", "0")]));
        assert_eq!(result, Err(invalid_seconds("COLLECTION_INTERVAL", "0")));
    }

    #[test]
    fn non_numeric_interval_is_rejected() {
        let result = Config::from_lookup(lookup(&[("COLLECTION_INTERVAL", "abc")]));
        assert_eq!(result, Err(invalid_seconds("COLLECTION_INTERVAL", "abc")));
    }

    #[test]
    fn negative_interval_is_rejected() {
        let result = Config::from_lookup(lookup(&[("COLLECTION_INTERVAL", "-3")]));
        assert_eq!(result, Err(invalid_seconds("COLLECTION_INTERVAL", "-3")));
    }

    #[test]
    fn empty_interval_is_rejected() {
        let result = Config::from_lookup(lookup(&[("COLLECTION_INTERVAL", "")]));
        assert_eq!(
            result,
            Err(ConfigError::Empty {
                var: "COLLECTION_INTERVAL"
            })
        );
    }

    #[test]
    fn zero_rpc_timeout_is_rejected() {
        let result = Config::from_lookup(lookup(&[("RPC_TIMEOUT", "0")]));
        assert_eq!(result, Err(invalid_seconds("RPC_TIMEOUT", "0")));
    }

    #[test]
    fn non_numeric_rpc_timeout_is_rejected() {
        let result = Config::from_lookup(lookup(&[("RPC_TIMEOUT", "fast")]));
        assert_eq!(result, Err(invalid_seconds("RPC_TIMEOUT", "fast")));
    }

    #[test]
    fn empty_rpc_timeout_is_rejected() {
        let result = Config::from_lookup(lookup(&[("RPC_TIMEOUT", " ")]));
        assert_eq!(result, Err(ConfigError::Empty { var: "RPC_TIMEOUT" }));
    }

    #[test]
    fn empty_output_file_is_rejected() {
        let result = Config::from_lookup(lookup(&[("OUTPUT_FILE", "")]));
        assert_eq!(result, Err(ConfigError::Empty { var: "OUTPUT_FILE" }));
    }
}
