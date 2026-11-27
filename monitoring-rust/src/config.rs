//! Shared monitoring configuration.
//!
//! Two independent configurations live here:
//!
//! * [`Config`] (Phases 2-3): single-node settings read from environment
//!   variables. A missing variable falls back to a default; a variable that is
//!   set but invalid is a hard error.
//! * [`NodesConfig`] (multi-node plan, Phase 1): the endpoints of the 15
//!   services (5 Geth + 5 Lighthouse Beacon Nodes + 5 Validator Clients),
//!   loaded from `config/nodes.toml`.

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;

pub const ENV_RPC_URL: &str = "RPC_URL";
pub const ENV_RPC_TIMEOUT: &str = "RPC_TIMEOUT";
pub const ENV_COLLECTION_INTERVAL: &str = "COLLECTION_INTERVAL";
pub const ENV_OUTPUT_FILE: &str = "OUTPUT_FILE";

pub const DEFAULT_RPC_URL: &str = "http://127.0.0.1:8545";
pub const DEFAULT_RPC_TIMEOUT_SECS: u64 = 3;
pub const DEFAULT_COLLECTION_INTERVAL_SECS: u64 = 5;
pub const DEFAULT_OUTPUT_FILE: &str = "data/observations.jsonl";

/// Environment variable overriding the path of the multi-node configuration file.
pub const ENV_NODES_CONFIG: &str = "NODES_CONFIG";
/// Default path of the multi-node configuration file.
pub const DEFAULT_NODES_CONFIG_PATH: &str = "config/nodes.toml";



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

// ---------------------------------------------------------------------------
// Multi-node configuration (config/nodes.toml)
// ---------------------------------------------------------------------------

/// Errors raised while loading or validating the multi-node configuration.
#[derive(Debug, Error)]
pub enum NodesConfigError {
    #[error("cannot read nodes configuration {path:?}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid nodes configuration: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("{var} is set but empty")]
    EmptyEnv { var: &'static str },

    #[error("nodes configuration must define at least one [[nodes]] entry")]
    NoNodes,

    #[error("invalid node id `{0}` (use only ASCII letters, digits, `-` and `_`)")]
    InvalidNodeId(String),

    #[error("duplicate node id `{0}`")]
    DuplicateNodeId(String),

    #[error(
        "node `{node}`: `{field}` must be an http:// or https:// URL with a host and no spaces (got `{value}`)"
    )]
    InvalidUrl {
        node: String,
        field: &'static str,
        value: String,
    },

    #[error("endpoint `{url}` is used more than once (node `{node}`, field `{field}`)")]
    DuplicateEndpoint {
        url: String,
        node: String,
        field: &'static str,
    },

    #[error("settings.{field} must be a strictly positive number of seconds")]
    InvalidSeconds { field: &'static str },
}

/// Timeouts and collection interval shared by every node.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonitoringSettings {
    /// Maximum duration of one HTTP request (JSON-RPC, Beacon REST, Prometheus).
    #[serde(default = "default_request_timeout_secs")]
    pub request_timeout_secs: u64,
    /// Delay between two collection cycles.
    #[serde(default = "default_collection_interval_secs")]
    pub collection_interval_secs: u64,
}

fn default_request_timeout_secs() -> u64 {
    DEFAULT_RPC_TIMEOUT_SECS
}

fn default_collection_interval_secs() -> u64 {
    DEFAULT_COLLECTION_INTERVAL_SECS
}

impl Default for MonitoringSettings {
    fn default() -> Self {
        Self {
            request_timeout_secs: DEFAULT_RPC_TIMEOUT_SECS,
            collection_interval_secs: DEFAULT_COLLECTION_INTERVAL_SECS,
        }
    }
}

impl MonitoringSettings {
    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.request_timeout_secs)
    }

    pub fn collection_interval(&self) -> Duration {
        Duration::from_secs(self.collection_interval_secs)
    }

    fn validate(&self) -> Result<(), NodesConfigError> {
        if self.request_timeout_secs == 0 {
            return Err(NodesConfigError::InvalidSeconds {
                field: "request_timeout_secs",
            });
        }
        if self.collection_interval_secs == 0 {
            return Err(NodesConfigError::InvalidSeconds {
                field: "collection_interval_secs",
            });
        }
        Ok(())
    }
}

/// Geth endpoints: JSON-RPC and Prometheus metrics.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GethEndpoints {
    pub rpc_url: String,
    pub metrics_url: String,
}

/// Lighthouse Beacon Node endpoints: Beacon REST API and Prometheus metrics.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeaconEndpoints {
    pub api_url: String,
    pub metrics_url: String,
}

/// Lighthouse Validator Client endpoints: Prometheus metrics and optional HTTP API.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatorEndpoints {
    pub metrics_url: String,
    #[serde(default)]
    pub api_url: Option<String>,
}

/// One node set: a Geth, its Lighthouse Beacon Node and its Validator Client.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeConfig {
    /// Unique identifier, copied into every observation (`node_id`).
    pub id: String,
    pub geth: GethEndpoints,
    pub beacon: BeaconEndpoints,
    pub validator: ValidatorEndpoints,
}

impl NodeConfig {
    /// Every configured endpoint as `(field name, url)`.
    fn endpoints(&self) -> Vec<(&'static str, &str)> {
        let mut endpoints = vec![
            ("geth.rpc_url", self.geth.rpc_url.as_str()),
            ("geth.metrics_url", self.geth.metrics_url.as_str()),
            ("beacon.api_url", self.beacon.api_url.as_str()),
            ("beacon.metrics_url", self.beacon.metrics_url.as_str()),
            ("validator.metrics_url", self.validator.metrics_url.as_str()),
        ];
        if let Some(api_url) = &self.validator.api_url {
            endpoints.push(("validator.api_url", api_url.as_str()));
        }
        endpoints
    }
}

/// Validated multi-node monitoring configuration (`config/nodes.toml`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodesConfig {
    #[serde(default)]
    pub settings: MonitoringSettings,
    #[serde(default)]
    pub nodes: Vec<NodeConfig>,
}

impl NodesConfig {
    /// Load the configuration from `NODES_CONFIG`, or `config/nodes.toml` if unset.
    pub fn from_env() -> Result<Self, NodesConfigError> {
        let raw = env::var_os(ENV_NODES_CONFIG).map(|v| v.to_string_lossy().into_owned());
        Self::from_file(parse_nodes_config_path(raw)?)
    }

    /// Load and validate the configuration from a TOML file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, NodesConfigError> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(|source| NodesConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_toml_str(&content)
    }

    /// Parse and validate the configuration from a TOML string.
    pub fn from_toml_str(content: &str) -> Result<Self, NodesConfigError> {
        let config: Self = toml::from_str(content)?;
        config.validate()?;
        Ok(config)
    }

    /// Find a node by its identifier.
    pub fn node(&self, id: &str) -> Option<&NodeConfig> {
        self.nodes.iter().find(|node| node.id == id)
    }

    fn validate(&self) -> Result<(), NodesConfigError> {
        self.settings.validate()?;
        if self.nodes.is_empty() {
            return Err(NodesConfigError::NoNodes);
        }

        let mut ids: HashSet<&str> = HashSet::new();
        let mut urls: HashSet<String> = HashSet::new();

        for node in &self.nodes {
            if !is_valid_node_id(&node.id) {
                return Err(NodesConfigError::InvalidNodeId(node.id.clone()));
            }
            if !ids.insert(node.id.as_str()) {
                return Err(NodesConfigError::DuplicateNodeId(node.id.clone()));
            }
            for (field, url) in node.endpoints() {
                if !is_valid_http_url(url) {
                    return Err(NodesConfigError::InvalidUrl {
                        node: node.id.clone(),
                        field,
                        value: url.to_string(),
                    });
                }
                // Two services must never share an endpoint (copy-paste guard).
                if !urls.insert(url.trim_end_matches('/').to_string()) {
                    return Err(NodesConfigError::DuplicateEndpoint {
                        url: url.to_string(),
                        node: node.id.clone(),
                        field,
                    });
                }
            }
        }
        Ok(())
    }
}

fn parse_nodes_config_path(raw: Option<String>) -> Result<PathBuf, NodesConfigError> {
    let Some(raw) = raw else {
        return Ok(PathBuf::from(DEFAULT_NODES_CONFIG_PATH));
    };
    let value = raw.trim();
    if value.is_empty() {
        return Err(NodesConfigError::EmptyEnv {
            var: ENV_NODES_CONFIG,
        });
    }
    Ok(PathBuf::from(value))
}

fn is_valid_node_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// An `http(s)://` URL with a non-empty host and no whitespace.
fn is_valid_http_url(value: &str) -> bool {
    if value.chars().any(char::is_whitespace) {
        return false;
    }
    value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .is_some_and(|host| !host.is_empty())
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

    // -----------------------------------------------------------------------
    // NodesConfig
    // -----------------------------------------------------------------------

    /// One `[[nodes]]` block; the five ports are `base..base + 5`.
    fn node_toml(id: &str, base: u16) -> String {
        format!(
            r#"
[[nodes]]
id = "{id}"

[nodes.geth]
rpc_url = "http://127.0.0.1:{}"
metrics_url = "http://127.0.0.1:{}/debug/metrics/prometheus"

[nodes.beacon]
api_url = "http://127.0.0.1:{}"
metrics_url = "http://127.0.0.1:{}/metrics"

[nodes.validator]
metrics_url = "http://127.0.0.1:{}/metrics"
"#,
            base,
            base + 1,
            base + 2,
            base + 3,
            base + 4
        )
    }

    fn parse(content: &str) -> Result<NodesConfig, NodesConfigError> {
        NodesConfig::from_toml_str(content)
    }

    #[test]
    fn shipped_nodes_toml_is_valid() {
        let config = parse(include_str!("../config/nodes.toml")).unwrap();
        let ids: Vec<&str> = config.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["node-01", "node-02", "node-03", "node-04", "node-05"]);
        assert_eq!(config.settings, MonitoringSettings::default());
        // 5 nodes x (2 Geth + 2 Beacon + 1 Validator metrics endpoint) = 25 endpoints.
        let total: usize = config.nodes.iter().map(|n| n.endpoints().len()).sum();
        assert_eq!(total, 25);
    }

    #[test]
    fn minimal_config_uses_default_settings() {
        let config = parse(&node_toml("node-01", 9000)).unwrap();
        assert_eq!(config.settings, MonitoringSettings::default());
        assert_eq!(config.settings.request_timeout(), Duration::from_secs(3));
        assert_eq!(
            config.settings.collection_interval(),
            Duration::from_secs(5)
        );
        assert_eq!(config.nodes.len(), 1);
    }

    #[test]
    fn custom_settings_are_applied() {
        let content = format!(
            "[settings]\nrequest_timeout_secs = 2\ncollection_interval_secs = 10\n{}",
            node_toml("node-01", 9000)
        );
        let config = parse(&content).unwrap();
        assert_eq!(config.settings.request_timeout(), Duration::from_secs(2));
        assert_eq!(
            config.settings.collection_interval(),
            Duration::from_secs(10)
        );
    }

    #[test]
    fn partial_settings_fall_back_to_defaults() {
        let content = format!(
            "[settings]\ncollection_interval_secs = 15\n{}",
            node_toml("node-01", 9000)
        );
        let config = parse(&content).unwrap();
        assert_eq!(
            config.settings.request_timeout_secs,
            DEFAULT_RPC_TIMEOUT_SECS
        );
        assert_eq!(config.settings.collection_interval_secs, 15);
    }

    #[test]
    fn validator_api_url_is_optional() {
        let config = parse(&node_toml("node-01", 9000)).unwrap();
        assert_eq!(config.nodes[0].validator.api_url, None);
        assert_eq!(config.nodes[0].endpoints().len(), 5);
    }

    #[test]
    fn validator_api_url_is_validated_when_present() {
        let content = node_toml("node-01", 9000).replace(
            "[nodes.validator]\n",
            "[nodes.validator]\napi_url = \"localhost:5062\"\n",
        );
        assert!(matches!(
            parse(&content),
            Err(NodesConfigError::InvalidUrl {
                field: "validator.api_url",
                ..
            })
        ));
    }

    #[test]
    fn node_lookup_by_id() {
        let content = format!(
            "{}{}",
            node_toml("node-01", 9000),
            node_toml("node-02", 9100)
        );
        let config = parse(&content).unwrap();
        assert_eq!(
            config.node("node-02").unwrap().geth.rpc_url,
            "http://127.0.0.1:9100"
        );
        assert!(config.node("node-99").is_none());
    }

    #[test]
    fn empty_file_has_no_nodes() {
        assert!(matches!(parse(""), Err(NodesConfigError::NoNodes)));
        assert!(matches!(
            parse("[settings]\nrequest_timeout_secs = 3\n"),
            Err(NodesConfigError::NoNodes)
        ));
    }

    #[test]
    fn malformed_toml_is_rejected() {
        assert!(matches!(
            parse("[[nodes]\nid = "),
            Err(NodesConfigError::Parse(_))
        ));
    }

    #[test]
    fn missing_section_is_rejected() {
        let content = "[[nodes]]\nid = \"node-01\"\n[nodes.geth]\nrpc_url = \"http://127.0.0.1:1\"\nmetrics_url = \"http://127.0.0.1:2\"\n";
        assert!(matches!(parse(content), Err(NodesConfigError::Parse(_))));
    }

    #[test]
    fn unknown_field_is_rejected() {
        let content = node_toml("node-01", 9000).replace(
            "[nodes.geth]\n",
            "[nodes.geth]\nrcp_url = \"http://127.0.0.1:1\"\n",
        );
        assert!(matches!(parse(&content), Err(NodesConfigError::Parse(_))));
        assert!(matches!(
            parse("[settings]\ntimeout = 3\n"),
            Err(NodesConfigError::Parse(_))
        ));
    }

    #[test]
    fn invalid_node_ids_are_rejected() {
        for id in ["", "node 01", "node/01", "nœud-01"] {
            let result = parse(&node_toml(id, 9000));
            assert!(
                matches!(&result, Err(NodesConfigError::InvalidNodeId(got)) if got == id),
                "id `{id}` should be rejected, got {result:?}"
            );
        }
    }

    #[test]
    fn duplicate_node_id_is_rejected() {
        let content = format!(
            "{}{}",
            node_toml("node-01", 9000),
            node_toml("node-01", 9100)
        );
        assert!(matches!(
            parse(&content),
            Err(NodesConfigError::DuplicateNodeId(id)) if id == "node-01"
        ));
    }

    #[test]
    fn invalid_urls_are_rejected() {
        for bad in [
            "127.0.0.1:8545",
            "http://",
            "http:///path",
            "ftp://127.0.0.1:8545",
            "http://127.0.0.1:8545 ",
            "http://127.0.0.1 :8545",
            "",
        ] {
            let content = node_toml("node-01", 9000).replace("http://127.0.0.1:9000", bad);
            let result = parse(&content);
            assert!(
                matches!(
                    &result,
                    Err(NodesConfigError::InvalidUrl { field: "geth.rpc_url", node, .. }) if node == "node-01"
                ),
                "url `{bad}` should be rejected, got {result:?}"
            );
        }
    }

    #[test]
    fn https_urls_are_accepted() {
        let content =
            node_toml("node-01", 9000).replace("http://127.0.0.1:9000", "https://geth.local:8545");
        assert!(parse(&content).is_ok());
    }

    #[test]
    fn duplicate_endpoint_across_nodes_is_rejected() {
        // Same ports for both nodes: every endpoint of node-02 duplicates node-01's.
        let content = format!(
            "{}{}",
            node_toml("node-01", 9000),
            node_toml("node-02", 9000)
        );
        assert!(matches!(
            parse(&content),
            Err(NodesConfigError::DuplicateEndpoint { node, field: "geth.rpc_url", .. }) if node == "node-02"
        ));
    }

    #[test]
    fn duplicate_endpoint_ignores_trailing_slash() {
        let second =
            node_toml("node-02", 9100).replace("http://127.0.0.1:9100", "http://127.0.0.1:9000/");
        let content = format!("{}{}", node_toml("node-01", 9000), second);
        assert!(matches!(
            parse(&content),
            Err(NodesConfigError::DuplicateEndpoint { .. })
        ));
    }

    #[test]
    fn zero_settings_are_rejected() {
        let timeout = format!(
            "[settings]\nrequest_timeout_secs = 0\n{}",
            node_toml("node-01", 9000)
        );
        assert!(matches!(
            parse(&timeout),
            Err(NodesConfigError::InvalidSeconds {
                field: "request_timeout_secs"
            })
        ));

        let interval = format!(
            "[settings]\ncollection_interval_secs = 0\n{}",
            node_toml("node-01", 9000)
        );
        assert!(matches!(
            parse(&interval),
            Err(NodesConfigError::InvalidSeconds {
                field: "collection_interval_secs"
            })
        ));
    }

    #[test]
    fn negative_settings_are_rejected() {
        let content = format!(
            "[settings]\nrequest_timeout_secs = -1\n{}",
            node_toml("node-01", 9000)
        );
        assert!(matches!(parse(&content), Err(NodesConfigError::Parse(_))));
    }

    #[test]
    fn from_file_reads_a_valid_file() {
        let path = env::temp_dir().join(format!("eva_nodes_ok_{}.toml", std::process::id()));
        fs::write(&path, node_toml("node-01", 9000)).unwrap();
        let result = NodesConfig::from_file(&path);
        fs::remove_file(&path).unwrap();
        assert_eq!(result.unwrap().nodes.len(), 1);
    }

    #[test]
    fn from_file_reports_a_missing_file() {
        let path = env::temp_dir().join("eva_nodes_does_not_exist.toml");
        assert!(matches!(
            NodesConfig::from_file(&path),
            Err(NodesConfigError::Read { .. })
        ));
    }

    #[test]
    fn nodes_config_path_resolution() {
        assert_eq!(
            parse_nodes_config_path(None).unwrap(),
            PathBuf::from("config/nodes.toml")
        );
        assert_eq!(
            parse_nodes_config_path(Some("  /etc/eva/nodes.toml ".to_string())).unwrap(),
            PathBuf::from("/etc/eva/nodes.toml")
        );
        assert!(matches!(
            parse_nodes_config_path(Some("  ".to_string())),
            Err(NodesConfigError::EmptyEnv {
                var: "NODES_CONFIG"
            })
        ));
    }
}
