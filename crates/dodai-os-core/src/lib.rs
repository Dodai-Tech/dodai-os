//! A small, portable app platform and launcher for ESP-Rust boards
//!
//! Keep the logic pure and testable: take data (and a `now` when time matters) and
//! return data. Every public item is documented (`missing_docs` warns, and the gate
//! runs `cargo doc` with `-D warnings`).

/// Splits a `key = value` line. Comments (`#`) and blank lines give `None`.
///
/// ```
/// assert_eq!(dodai_os_core::parse_kv(" a = b "), Some(("a", "b")));
/// assert_eq!(dodai_os_core::parse_kv("# note"), None);
/// ```
#[must_use]
pub fn parse_kv(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (k, v) = line.split_once('=')?;
    let k = k.trim();
    (!k.is_empty()).then(|| (k, v.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_and_trims() {
        assert_eq!(parse_kv(" a = b "), Some(("a", "b")));
        assert_eq!(parse_kv("a=b=c"), Some(("a", "b=c")));
    }

    #[test]
    fn skips_comments_blanks_and_empty_keys() {
        assert_eq!(parse_kv("# a = b"), None);
        assert_eq!(parse_kv("   "), None);
        assert_eq!(parse_kv(" = b"), None);
        assert_eq!(parse_kv("no equals"), None);
    }
}
