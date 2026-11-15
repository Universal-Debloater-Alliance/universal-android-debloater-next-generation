//! The one place that builds HTTP clients, so every request gets the same safety settings.

use std::time::Duration;

/// Time allowed for DNS, TCP and the TLS handshake.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// An HTTPS-only client whose calls fail after `total_timeout`.
///
/// `ureq` has no timeout by default, so a stalled server would block the
/// caller forever (CWE-400). `https_only` refuses a redirect to plain HTTP.
pub fn agent(total_timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(true)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_global(Some(total_timeout))
        .build()
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_http_is_refused_without_network_access() {
        let result = agent(Duration::from_secs(5))
            .get("http://example.invalid/")
            .call();
        assert!(matches!(result, Err(ureq::Error::RequireHttpsOnly(_))));
    }
}
