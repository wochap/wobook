//! Title and description fetch (D8).

use std::time::Duration;

use scraper::{Html, Selector};

#[derive(Debug, Clone, Copy)]
pub struct FetchLimits {
    pub timeout: Duration,
    pub max_redirects: usize,
    pub max_body: usize,
}

impl Default for FetchLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(8),
            max_redirects: 5,
            max_body: 512 * 1024,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Metadata {
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("http status {0}")]
    Status(u16),
    #[error("not html: {0}")]
    NotHtml(String),
}

const TITLE_MAX: usize = 512;
const DESC_MAX: usize = 4096;

pub async fn fetch_metadata(url: &str, limits: FetchLimits) -> Result<Metadata, FetchError> {
    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(limits.timeout)
        .redirect(reqwest::redirect::Policy::limited(limits.max_redirects))
        .user_agent(format!("wobook/{}", crate::VERSION))
        .build()?;
    let mut response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(FetchError::Status(response.status().as_u16()));
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mime = content_type.split(';').next().unwrap_or("").trim();
    if mime != "text/html" && mime != "application/xhtml+xml" {
        return Err(FetchError::NotHtml(mime.to_string()));
    }
    let header_charset = charset_param(&content_type);
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        let room = limits.max_body.saturating_sub(body.len());
        body.extend_from_slice(&chunk[..chunk.len().min(room)]);
        if body.len() >= limits.max_body {
            break;
        }
    }
    let html = decode(&body, header_charset.as_deref());
    Ok(extract(&html))
}

fn charset_param(content_type: &str) -> Option<String> {
    content_type.split(';').skip(1).find_map(|p| {
        let (k, v) = p.split_once('=')?;
        (k.trim() == "charset").then(|| v.trim().trim_matches('"').to_string())
    })
}

fn meta_charset(body: &[u8]) -> Option<String> {
    let head = String::from_utf8_lossy(&body[..body.len().min(4096)]).to_ascii_lowercase();
    let idx = head.find("charset=")?;
    let rest = head[idx + 8..].trim_start_matches(['"', '\'']);
    let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))?;
    Some(rest[..end].to_string())
}

fn decode(body: &[u8], header_charset: Option<&str>) -> String {
    let label = header_charset
        .map(str::to_string)
        .or_else(|| meta_charset(body))
        .unwrap_or_else(|| "utf-8".into());
    let encoding = encoding_rs::Encoding::for_label(label.as_bytes()).unwrap_or(encoding_rs::UTF_8);
    encoding.decode(body).0.into_owned()
}

/// Collapses whitespace, trims, and caps at `max` chars.
pub fn sanitize(text: &str, max: usize) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max)
        .collect()
}

fn first_text(doc: &Html, selector: &str) -> Option<String> {
    let sel = Selector::parse(selector).ok()?;
    doc.select(&sel).find_map(|e| {
        let text = e.text().collect::<String>();
        (!text.trim().is_empty()).then_some(text)
    })
}

fn first_attr(doc: &Html, selector: &str) -> Option<String> {
    let sel = Selector::parse(selector).ok()?;
    doc.select(&sel).find_map(|e| {
        e.value()
            .attr("content")
            .filter(|c| !c.trim().is_empty())
            .map(str::to_string)
    })
}

pub fn extract(html: &str) -> Metadata {
    let doc = Html::parse_document(html);
    let title = first_text(&doc, "title")
        .or_else(|| first_attr(&doc, r#"meta[property="og:title"]"#))
        .map(|t| sanitize(&t, TITLE_MAX))
        .filter(|t| !t.is_empty());
    let description = first_attr(&doc, r#"meta[name="description"]"#)
        .or_else(|| first_attr(&doc, r#"meta[property="og:description"]"#))
        .map(|t| sanitize(&t, DESC_MAX))
        .filter(|t| !t.is_empty());
    Metadata { title, description }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn serve(response: Vec<u8>, delay: Duration) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut sock, _)) = listener.accept().await {
                let response = response.clone();
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let _ = sock.read(&mut buf).await;
                    tokio::time::sleep(delay).await;
                    let _ = sock.write_all(&response).await;
                    let _ = sock.shutdown().await;
                });
            }
        });
        format!("http://{addr}/")
    }

    fn http(ct: &str, body: &[u8]) -> Vec<u8> {
        let mut out = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        out.extend_from_slice(body);
        out
    }

    #[tokio::test]
    async fn html_fills_fields() {
        let url = serve(
            http(
                "text/html; charset=utf-8",
                b"<html><head><title>\n Hello \n  World </title><meta name=\"description\" content=\"Desc\"></head></html>",
            ),
            Duration::ZERO,
        )
        .await;
        let meta = fetch_metadata(&url, FetchLimits::default()).await.unwrap();
        assert_eq!(meta.title.as_deref(), Some("Hello World"));
        assert_eq!(meta.description.as_deref(), Some("Desc"));
    }

    #[tokio::test]
    async fn og_fallback_and_latin1() {
        let url = serve(
            http(
                "text/html; charset=iso-8859-1",
                b"<html><head><meta property=\"og:title\" content=\"Caf\xe9\"></head></html>",
            ),
            Duration::ZERO,
        )
        .await;
        let meta = fetch_metadata(&url, FetchLimits::default()).await.unwrap();
        assert_eq!(meta.title.as_deref(), Some("Café"));
    }

    #[tokio::test]
    async fn non_html_rejected() {
        let url = serve(http("application/pdf", b"%PDF"), Duration::ZERO).await;
        assert!(matches!(
            fetch_metadata(&url, FetchLimits::default()).await,
            Err(FetchError::NotHtml(_))
        ));
    }

    #[tokio::test]
    async fn timeout_fails() {
        let url = serve(
            http("text/html", b"<title>x</title>"),
            Duration::from_secs(5),
        )
        .await;
        let limits = FetchLimits {
            timeout: Duration::from_millis(200),
            ..FetchLimits::default()
        };
        assert!(fetch_metadata(&url, limits).await.is_err());
    }

    #[tokio::test]
    async fn oversized_body_truncated() {
        let mut body = b"<html><head><title>Big</title></head><body>".to_vec();
        body.extend(std::iter::repeat_n(b'a', 2 * 1024 * 1024));
        let url = serve(http("text/html", &body), Duration::ZERO).await;
        let limits = FetchLimits {
            max_body: 1024,
            ..FetchLimits::default()
        };
        let meta = fetch_metadata(&url, limits).await.unwrap();
        assert_eq!(meta.title.as_deref(), Some("Big"));
    }

    #[test]
    fn caps_lengths() {
        let long = "a".repeat(1000);
        assert_eq!(sanitize(&long, TITLE_MAX).len(), 512);
    }
}
