//! Label-based discovery of Kurtosis Ethereum component containers.
//!
//! The service name is useful in errors but is deliberately not used for the
//! association: Kurtosis recreates it with a new suffix. The Ethereum package
//! labels survive that recreation and identify both the node index and role.

use std::collections::{BTreeMap, HashMap};

use thiserror::Error;

use super::docker_client::DockerContainer;

pub const NODE_INDEX_LABEL: &str = "com.kurtosistech.custom.ethereum-package.node-index";
pub const CLIENT_TYPE_LABEL: &str = "com.kurtosistech.custom.ethereum-package.client-type";
pub const CLIENT_LABEL: &str = "com.kurtosistech.custom.ethereum-package.client";

/// The monitored role of an Ethereum-package service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ComponentRole {
    Geth,
    Beacon,
    Validator,
}

impl ComponentRole {
    pub const ALL: [Self; 3] = [Self::Geth, Self::Beacon, Self::Validator];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Geth => "geth",
            Self::Beacon => "beacon",
            Self::Validator => "validator",
        }
    }
}

/// A discovered component container. `host_pid` is filled after inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredContainer {
    pub node_id: String,
    pub role: ComponentRole,
    pub container_id: String,
    pub container_name: Option<String>,
    pub host_pid: Option<u64>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DiscoveryError {
    #[error("container `{container}` has incomplete Ethereum-package labels: missing `{label}`")]
    MissingLabel {
        container: String,
        label: &'static str,
    },

    #[error("container `{container}` has invalid node index `{value}`")]
    InvalidNodeIndex { container: String, value: String },

    #[error("container `{container}` has unsupported Ethereum client type `{client_type}` with client `{client}`")]
    UnsupportedComponent {
        container: String,
        client_type: String,
        client: String,
    },

    #[error("node `{node_id}` has more than one {role} container: {containers:?}")]
    DuplicateComponent {
        node_id: String,
        role: &'static str,
        containers: Vec<String>,
    },

    #[error("node `{node_id}` is missing its {role} container")]
    MissingComponent { node_id: String, role: &'static str },
}

/// Validates and maps Docker summaries to logical node/component containers.
///
/// Services unrelated to the Ethereum package have none of its three labels
/// and are ignored. A partial label set is an error rather than a guess.
pub fn discover(
    containers: Vec<DockerContainer>,
) -> Result<Vec<DiscoveredContainer>, DiscoveryError> {
    let mut grouped: BTreeMap<String, BTreeMap<ComponentRole, Vec<DiscoveredContainer>>> =
        BTreeMap::new();

    for container in containers {
        let Some((node_id, role)) = classify(&container)? else {
            continue;
        };

        grouped
            .entry(node_id.clone())
            .or_default()
            .entry(role)
            .or_default()
            .push(DiscoveredContainer {
                node_id,
                role,
                container_id: container.id,
                container_name: container.name,
                host_pid: None,
            });
    }

    let mut result = Vec::new();
    for (node_id, roles) in grouped {
        for role in ComponentRole::ALL {
            let containers = roles.get(&role).map(Vec::as_slice).unwrap_or_default();
            match containers {
                [] => {
                    return Err(DiscoveryError::MissingComponent {
                        node_id,
                        role: role.as_str(),
                    });
                }
                [container] => result.push(container.clone()),
                many => {
                    return Err(DiscoveryError::DuplicateComponent {
                        node_id,
                        role: role.as_str(),
                        containers: many.iter().map(container_display_from_discovered).collect(),
                    });
                }
            }
        }
    }

    Ok(result)
}

/// The `(node_id, role)` an Ethereum-package container belongs to, from its
/// labels only (`node-index` → `node-NN`, `client-type`/`client` → role).
///
/// `Ok(None)` for a service without any Ethereum-package label (genesis
/// helpers, key generation, ...). A partial or invalid label set is an error
/// rather than a guess.
pub fn classify(
    container: &DockerContainer,
) -> Result<Option<(String, ComponentRole)>, DiscoveryError> {
    let labels = &container.labels;
    let has_ethereum_label = [NODE_INDEX_LABEL, CLIENT_TYPE_LABEL, CLIENT_LABEL]
        .iter()
        .any(|label| labels.contains_key(*label));
    if !has_ethereum_label {
        return Ok(None);
    }

    let display = container_display(container);
    let node_index = required_label(labels, NODE_INDEX_LABEL, &display)?;
    let client_type = required_label(labels, CLIENT_TYPE_LABEL, &display)?;
    let client = required_label(labels, CLIENT_LABEL, &display)?;
    let node_number = node_index
        .parse::<u32>()
        .ok()
        .filter(|number| *number > 0)
        .ok_or_else(|| DiscoveryError::InvalidNodeIndex {
            container: display.clone(),
            value: node_index.to_string(),
        })?;
    let role = component_role(client_type, client, &display)?;

    Ok(Some((format!("node-{node_number:02}"), role)))
}

fn required_label<'a>(
    labels: &'a HashMap<String, String>,
    label: &'static str,
    container: &str,
) -> Result<&'a str, DiscoveryError> {
    labels
        .get(label)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| DiscoveryError::MissingLabel {
            container: container.to_string(),
            label,
        })
}

fn component_role(
    client_type: &str,
    client: &str,
    container: &str,
) -> Result<ComponentRole, DiscoveryError> {
    match (client_type, client) {
        ("execution", "geth") => Ok(ComponentRole::Geth),
        ("beacon", "lighthouse") => Ok(ComponentRole::Beacon),
        ("validator", "lighthouse") => Ok(ComponentRole::Validator),
        _ => Err(DiscoveryError::UnsupportedComponent {
            container: container.to_string(),
            client_type: client_type.to_string(),
            client: client.to_string(),
        }),
    }
}

fn container_display(container: &DockerContainer) -> String {
    container
        .name
        .clone()
        .unwrap_or_else(|| container.id.clone())
}

fn container_display_from_discovered(container: &DiscoveredContainer) -> String {
    container
        .container_name
        .clone()
        .unwrap_or_else(|| container.container_id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn container(index: u8, client_type: &str, client: &str) -> DockerContainer {
        let prefix = match client_type {
            "execution" => "el",
            "beacon" => "cl",
            "validator" => "vc",
            _ => "unknown",
        };
        DockerContainer {
            id: format!("{prefix}-{index}-{client_type}"),
            name: Some(format!("{prefix}-{index}-{client}")),
            labels: HashMap::from([
                (NODE_INDEX_LABEL.to_string(), index.to_string()),
                (CLIENT_TYPE_LABEL.to_string(), client_type.to_string()),
                (CLIENT_LABEL.to_string(), client.to_string()),
            ]),
            ..DockerContainer::default()
        }
    }

    fn all_five_nodes() -> Vec<DockerContainer> {
        (1..=5)
            .flat_map(|index| {
                [
                    container(index, "execution", "geth"),
                    container(index, "beacon", "lighthouse"),
                    container(index, "validator", "lighthouse"),
                ]
            })
            .collect()
    }

    #[test]
    fn discovers_and_maps_all_fifteen_component_containers() {
        let discovered = discover(all_five_nodes()).expect("valid mapping");

        assert_eq!(discovered.len(), 15);
        for index in 1..=5 {
            let node = format!("node-{index:02}");
            let roles: Vec<_> = discovered
                .iter()
                .filter(|container| container.node_id == node)
                .map(|container| container.role)
                .collect();
            assert_eq!(
                roles,
                [
                    ComponentRole::Geth,
                    ComponentRole::Beacon,
                    ComponentRole::Validator
                ]
            );
        }
    }

    #[test]
    fn rejects_a_missing_component() {
        let mut containers = all_five_nodes();
        containers.retain(|container| container.id != "vc-3-validator");

        assert_eq!(
            discover(containers),
            Err(DiscoveryError::MissingComponent {
                node_id: "node-03".to_string(),
                role: "validator",
            })
        );
    }

    #[test]
    fn rejects_an_ambiguous_component() {
        let mut containers = all_five_nodes();
        let mut duplicate = container(2, "execution", "geth");
        duplicate.id = "second-geth".to_string();
        duplicate.name = Some("el-2-geth-recreated".to_string());
        containers.push(duplicate);

        assert!(matches!(
            discover(containers),
            Err(DiscoveryError::DuplicateComponent { node_id, role: "geth", .. }) if node_id == "node-02"
        ));
    }

    #[test]
    fn rejects_partial_or_invalid_ethereum_metadata() {
        let mut partial = container(1, "execution", "geth");
        partial.labels.remove(CLIENT_LABEL);
        assert!(matches!(
            discover(vec![partial]),
            Err(DiscoveryError::MissingLabel {
                label: CLIENT_LABEL,
                ..
            })
        ));

        let mut invalid = container(1, "execution", "geth");
        invalid
            .labels
            .insert(NODE_INDEX_LABEL.to_string(), "zero".to_string());
        assert!(matches!(
            discover(vec![invalid]),
            Err(DiscoveryError::InvalidNodeIndex { value, .. }) if value == "zero"
        ));
    }

    #[test]
    fn ignores_non_ethereum_kurtosis_services() {
        let unrelated = DockerContainer {
            id: "artifact-expander".to_string(),
            name: Some("files-artifacts-expander".to_string()),
            ..DockerContainer::default()
        };

        assert_eq!(
            discover(vec![unrelated]).unwrap(),
            Vec::<DiscoveredContainer>::new()
        );
    }
}
