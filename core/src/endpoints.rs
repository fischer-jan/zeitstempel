//! Network endpoints — the Bitcoin block APIs and the calendar servers.
//!
//! Tests replace these with a local fixture server through two
//! environment variables. An override must point at loopback
//! (`http://127.0.0.1:<port>`); anything else is an error, so a typo
//! can never silently send a test to the live network or let the
//! environment redirect verification to a remote server.

/// Overrides the Bitcoin block APIs (comma-separated base URLs).
pub const BITCOIN_API_ENV: &str = "ZEITSTEMPEL_BITCOIN_API";

/// Overrides the calendar servers used by `stamp` (comma-separated).
pub const CALENDARS_ENV: &str = "ZEITSTEMPEL_CALENDARS";

/// Blockstream.info first, mempool.space as fallback.
const BITCOIN_APIS: &[&str] = &["https://blockstream.info/api", "https://mempool.space/api"];

/// Calendar servers to submit to (first two for speed).
const CALENDAR_SERVERS: &[&str] = &[
    "https://alice.btc.calendar.opentimestamps.org",
    "https://bob.btc.calendar.opentimestamps.org",
];

/// Base URLs of the Bitcoin block APIs, in the order to try them.
pub fn bitcoin_apis() -> Result<Vec<String>, String> {
    resolve(
        BITCOIN_API_ENV,
        std::env::var(BITCOIN_API_ENV).ok(),
        BITCOIN_APIS,
    )
}

/// Calendar server URLs to submit new digests to.
pub fn calendars() -> Result<Vec<String>, String> {
    resolve(
        CALENDARS_ENV,
        std::env::var(CALENDARS_ENV).ok(),
        CALENDAR_SERVERS,
    )
}

/// Use the override when one is set, else the built-in defaults.
/// With an override there is no fallback to the defaults.
fn resolve(var: &str, value: Option<String>, defaults: &[&str]) -> Result<Vec<String>, String> {
    let Some(value) = value else {
        return Ok(defaults.iter().map(|s| s.to_string()).collect());
    };
    let urls: Vec<String> = value
        .split(',')
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if urls.is_empty() {
        return Err(format!("{} is set but empty", var));
    }
    for url in &urls {
        // The authority must be exactly 127.0.0.1:<digits>, so tricks
        // like "http://127.0.0.1:80@evil.example" are rejected too.
        let port = url
            .strip_prefix("http://127.0.0.1:")
            .map(|rest| rest.split('/').next().unwrap_or(""));
        let is_loopback =
            matches!(port, Some(p) if !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
        if !is_loopback {
            return Err(format!(
                "{} may only point at http://127.0.0.1:<port> (test fixture server), got '{}'",
                var, url
            ));
        }
    }
    Ok(urls)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults_without_override() {
        let urls = resolve("X", None, BITCOIN_APIS).unwrap();
        assert_eq!(
            urls,
            vec!["https://blockstream.info/api", "https://mempool.space/api"]
        );
    }

    #[test]
    fn test_loopback_override_replaces_defaults() {
        let urls = resolve(
            "X",
            Some("http://127.0.0.1:4000/, http://127.0.0.1:4001".into()),
            CALENDAR_SERVERS,
        )
        .unwrap();
        assert_eq!(urls, vec!["http://127.0.0.1:4000", "http://127.0.0.1:4001"]);
    }

    #[test]
    fn test_override_rejects_non_loopback() {
        for bad in [
            "https://evil.example/api",
            "http://localhost:4000",
            "http://127.0.0.1.evil.example:80",
            "http://127.0.0.1:80@evil.example",
            "http://127.0.0.1:",
            "http://127.0.0.1:4000,https://evil.example",
            "",
            " , ",
        ] {
            assert!(
                resolve("X", Some(bad.into()), BITCOIN_APIS).is_err(),
                "should reject '{}'",
                bad
            );
        }
    }
}
