use reqwest::Client;
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidatorError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Invalid Prometheus metrics format: {0}")]
    InvalidMetrics(String),
}

#[derive(Debug, Clone)]
pub struct PrometheusMetric {
    pub name: String,
    pub labels: HashMap<String, String>,
    pub value: f64,
}

#[derive(Debug, Clone)]
pub struct ValidatorClient {
    client: Client,
    metrics_url: String,
}

impl ValidatorClient {
    /// Creates a Validator Prometheus client for one node.
    pub fn new(
        metrics_url: impl Into<String>,
        request_timeout_secs: u64,
    ) -> Result<Self, ValidatorError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(request_timeout_secs))
            .build()?;

        Ok(Self {
            client,
            metrics_url: metrics_url.into(),
        })
    }

    /// Retrieves the raw Prometheus metrics.
    pub async fn get_metrics(&self) -> Result<String, ValidatorError> {
        let response = self
            .client
            .get(&self.metrics_url)
            .send()
            .await?
            .error_for_status()?;

        Ok(response.text().await?)
    }

    /// Retrieves and parses Prometheus metrics.
    pub async fn collect_metrics(&self) -> Result<Vec<PrometheusMetric>, ValidatorError> {
        let raw_metrics = self.get_metrics().await?;

        parse_prometheus_metrics(&raw_metrics)
    }

    /// Returns metrics matching a specific name.
    pub async fn get_metric(
        &self,
        metric_name: &str,
    ) -> Result<Vec<PrometheusMetric>, ValidatorError> {
        let metrics = self.collect_metrics().await?;

        Ok(metrics
            .into_iter()
            .filter(|metric| metric.name == metric_name)
            .collect())
    }

    /// Returns the configured Prometheus endpoint.
    pub fn metrics_url(&self) -> &str {
        &self.metrics_url
    }
}

/// Parses the basic Prometheus text exposition format.
///
/// Supports:
/// metric_name{label="value"} 123.45
/// metric_name 123.45
///
/// Comments and HELP/TYPE declarations are ignored.
pub fn parse_prometheus_metrics(input: &str) -> Result<Vec<PrometheusMetric>, ValidatorError> {
    let mut metrics = Vec::new();

    for line in input.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let (metric_part, value_part) = line
            .rsplit_once(char::is_whitespace)
            .ok_or_else(|| ValidatorError::InvalidMetrics(line.to_string()))?;

        let value: f64 = value_part
            .parse()
            .map_err(|_| ValidatorError::InvalidMetrics(line.to_string()))?;

        let (name, labels) = parse_metric_name_and_labels(metric_part)?;

        metrics.push(PrometheusMetric {
            name,
            labels,
            value,
        });
    }

    Ok(metrics)
}

fn parse_metric_name_and_labels(
    input: &str,
) -> Result<(String, HashMap<String, String>), ValidatorError> {
    let Some(start) = input.find('{') else {
        return Ok((input.to_string(), HashMap::new()));
    };

    if !input.ends_with('}') {
        return Err(ValidatorError::InvalidMetrics(input.to_string()));
    }

    let name = input[..start].to_string();
    let labels_part = &input[start + 1..input.len() - 1];

    let mut labels = HashMap::new();

    for label in labels_part.split(',') {
        let Some((key, value)) = label.split_once('=') else {
            return Err(ValidatorError::InvalidMetrics(input.to_string()));
        };

        let value = value.trim().trim_matches('"').to_string();

        labels.insert(key.trim().to_string(), value);
    }

    Ok((name, labels))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_metric() {
        let input = "validator_attestations_total 42";

        let metrics = parse_prometheus_metrics(input).unwrap();

        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "validator_attestations_total");
        assert_eq!(metrics[0].value, 42.0);
    }

    #[test]
    fn parses_metric_with_labels() {
        let input = r#"async_tasks_count{async_task_count="attestation_service"} 1"#;

        let metrics = parse_prometheus_metrics(input).unwrap();

        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "async_tasks_count");
        assert_eq!(
            metrics[0].labels.get("async_task_count"),
            Some(&"attestation_service".to_string())
        );
        assert_eq!(metrics[0].value, 1.0);
    }

    #[test]
    fn ignores_prometheus_comments() {
        let input = r#"
# HELP validator_attestations_total Total attestations
# TYPE validator_attestations_total counter
validator_attestations_total 42
"#;

        let metrics = parse_prometheus_metrics(input).unwrap();

        assert_eq!(metrics.len(), 1);
    }
}
