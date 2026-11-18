pub mod attestation_inclusion;
pub mod connections;
pub mod consensus;
pub mod diversity;
pub mod peers;
pub mod propagation;
pub mod sync;

pub use attestation_inclusion::AttestationInclusionCollector;
pub use connections::ConnectionsCollector;
pub use consensus::ConsensusCollector;
pub use diversity::DiversityCollector;
pub use peers::PeersCollector;
pub use propagation::PropagationCollector;
pub use sync::SyncCollector;

/// Parses an unsigned integer returned by the Beacon API.
///
/// The Beacon API encodes every uint64 as a decimal string. Strict on
/// purpose: ASCII digits only, so signs, whitespace and hexadecimal
/// notation are rejected, as is any value that overflows u64.
pub(crate) fn parse_decimal_u64(value: &str) -> Option<u64> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }

    value.parse::<u64>().ok()
}

/// Same as `parse_decimal_u64` for an optional field.
pub(crate) fn parse_optional_decimal(value: Option<&str>) -> Option<u64> {
    value.and_then(parse_decimal_u64)
}

/// Helpers shared by the HTTP-level tests of the Beacon collectors.
#[cfg(test)]
pub(crate) mod test_support {
    use crate::clients::beacon_client::BeaconClient;

    use serde_json::Value;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Beacon client pointing at the mock server.
    pub(crate) fn client_for(server: &MockServer) -> BeaconClient {
        BeaconClient::new(server.uri(), 5).expect("client should build")
    }

    /// Beacon client for a port where nothing is listening.
    pub(crate) fn unreachable_client() -> BeaconClient {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        BeaconClient::new(format!("http://127.0.0.1:{port}"), 2).expect("client should build")
    }

    /// Answers GET `route` with HTTP 200 and the given JSON body.
    pub(crate) async fn mock_json(server: &MockServer, route: &str, body: Value) {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(server)
            .await;
    }

    /// Answers GET `route` with an empty body and the given status.
    pub(crate) async fn mock_status(server: &MockServer, route: &str, status: u16) {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(status))
            .mount(server)
            .await;
    }

    /// Answers GET `route` with HTTP 200 and a body that is not valid JSON.
    pub(crate) async fn mock_garbage(server: &MockServer, route: &str) {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>oops</html>"))
            .mount(server)
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_regular_values() {
        assert_eq!(parse_decimal_u64("0"), Some(0));
        assert_eq!(parse_decimal_u64("1200"), Some(1200));
        assert_eq!(parse_decimal_u64("007"), Some(7));
    }

    #[test]
    fn parses_u64_max_and_rejects_overflow() {
        assert_eq!(parse_decimal_u64("18446744073709551615"), Some(u64::MAX));
        assert_eq!(parse_decimal_u64("18446744073709551616"), None);
    }

    #[test]
    fn rejects_empty_and_non_digit_values() {
        assert_eq!(parse_decimal_u64(""), None);
        assert_eq!(parse_decimal_u64("invalid"), None);
        assert_eq!(parse_decimal_u64("1.5"), None);
        assert_eq!(parse_decimal_u64("0x10"), None);
    }

    #[test]
    fn rejects_signs_and_whitespace() {
        // `str::parse::<u64>` alone would accept the leading '+'.
        assert_eq!(parse_decimal_u64("+5"), None);
        assert_eq!(parse_decimal_u64("-5"), None);
        assert_eq!(parse_decimal_u64(" 5"), None);
        assert_eq!(parse_decimal_u64("5 "), None);
    }

    #[test]
    fn optional_parser_handles_missing_values() {
        assert_eq!(parse_optional_decimal(Some("15")), Some(15));
        assert_eq!(parse_optional_decimal(Some("x")), None);
        assert_eq!(parse_optional_decimal(None), None);
    }
}
