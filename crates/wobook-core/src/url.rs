//! URL normalization: the normalized URL is the bookmark identity (D2).

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid url: {0}")]
pub struct UrlError(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NormalizedUrl(String);

impl NormalizedUrl {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for NormalizedUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

const OTHER_SCHEMES: &[&str] = &["ftp", "file", "magnet"];

pub fn normalize(input: &str) -> Result<NormalizedUrl, UrlError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(UrlError("empty url".into()));
    }
    if trimmed.chars().any(char::is_whitespace) {
        return Err(UrlError(trimmed.to_string()));
    }
    let lower = trimmed.to_ascii_lowercase();
    let has_scheme = lower.starts_with("http://")
        || lower.starts_with("https://")
        || OTHER_SCHEMES
            .iter()
            .any(|s| lower.starts_with(&format!("{s}:")));
    let candidate = if has_scheme {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let mut parsed = ::url::Url::parse(&candidate).map_err(|_| UrlError(trimmed.to_string()))?;
    let scheme = parsed.scheme().to_string();
    if scheme == "http" || scheme == "https" {
        let Some(host) = parsed.host_str() else {
            return Err(UrlError(trimmed.to_string()));
        };
        // Bare words like "foo" are not URLs; require a dot, localhost or an IP.
        let is_ip = matches!(
            parsed.host(),
            Some(::url::Host::Ipv4(_) | ::url::Host::Ipv6(_))
        );
        if !has_scheme && !is_ip && !host.contains('.') && host != "localhost" {
            return Err(UrlError(trimmed.to_string()));
        }
    } else if !OTHER_SCHEMES.contains(&scheme.as_str()) {
        return Err(UrlError(trimmed.to_string()));
    }
    // `url` already lowercases scheme and host and drops default ports.
    if let Some(fragment) = parsed.fragment()
        && !fragment.starts_with('!')
    {
        parsed.set_fragment(None);
    }
    Ok(NormalizedUrl(parsed.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str) -> String {
        normalize(s).unwrap().into_string()
    }

    #[test]
    fn equivalent_inputs_collapse() {
        assert_eq!(
            n("HTTPS://Example.com:443/docs#intro"),
            "https://example.com/docs"
        );
        assert_eq!(n("example.com/docs"), "https://example.com/docs");
        assert_eq!(n("  http://Example.COM:80/x "), "http://example.com/x");
    }

    #[test]
    fn query_and_trailing_slash_preserved() {
        assert_eq!(
            n("https://example.com/a/?q=1&utm_source=x"),
            "https://example.com/a/?q=1&utm_source=x"
        );
    }

    #[test]
    fn hashbang_kept() {
        assert_eq!(
            n("https://example.com/app#!/route"),
            "https://example.com/app#!/route"
        );
    }

    #[test]
    fn non_default_port_kept() {
        assert_eq!(n("http://localhost:8080/"), "http://localhost:8080/");
    }

    #[test]
    fn invalid_rejected() {
        assert!(normalize("").is_err());
        assert!(normalize("   ").is_err());
        assert!(normalize("not a url at all").is_err());
        assert!(normalize("javascript:alert(1)").is_err());
    }

    #[test]
    fn other_schemes_allowed() {
        assert_eq!(n("ftp://Example.com/f"), "ftp://example.com/f");
        assert!(n("magnet:?xt=urn:btih:abc").starts_with("magnet:"));
    }
}
