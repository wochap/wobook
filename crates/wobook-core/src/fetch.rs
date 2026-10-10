//! Title and description fetch (D8) and favicon discovery.

use std::time::Duration;

use scraper::{Html, Selector};
use url::Url;

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

fn client(limits: FetchLimits) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(limits.timeout)
        .redirect(reqwest::redirect::Policy::limited(limits.max_redirects))
        .user_agent(format!("wobook/{}", crate::VERSION))
        .build()
}

/// Fetches an HTML page, returning the decoded body and the final URL after redirects.
async fn fetch_html(
    client: &reqwest::Client,
    url: &str,
    limits: FetchLimits,
) -> Result<(String, Url), FetchError> {
    let mut response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(FetchError::Status(response.status().as_u16()));
    }
    let final_url = response.url().clone();
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
    Ok((decode(&body, header_charset.as_deref()), final_url))
}

pub async fn fetch_metadata(url: &str, limits: FetchLimits) -> Result<Metadata, FetchError> {
    let client = client(limits)?;
    let (html, _) = fetch_html(&client, url, limits).await?;
    Ok(extract(&html))
}

const ICON_MAX: usize = 256 * 1024;
const ICON_TARGET: u32 = 48;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconCandidate {
    pub url: Url,
    /// Largest declared dimension, `None` when `sizes` is missing or `any`.
    pub size: Option<u32>,
    pub mime: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Icon {
    pub bytes: Vec<u8>,
    pub mime: String,
}

/// Reads `<link rel="icon">`-like entries, resolving hrefs against `base`. SVG icons are dropped.
pub fn extract_icons(html: &str, base: &Url) -> Vec<IconCandidate> {
    let doc = Html::parse_document(html);
    let Ok(sel) = Selector::parse("link[rel][href]") else {
        return Vec::new();
    };
    doc.select(&sel)
        .filter_map(|e| {
            let el = e.value();
            let rel = el.attr("rel")?.to_ascii_lowercase();
            if !rel
                .split_ascii_whitespace()
                .any(|t| t == "icon" || t == "apple-touch-icon")
            {
                return None;
            }
            let url = base.join(el.attr("href")?.trim()).ok()?;
            let mime = el
                .attr("type")
                .map(|t| t.trim().to_ascii_lowercase())
                .filter(|t| !t.is_empty());
            if mime.as_deref() == Some("image/svg+xml")
                || url.path().to_ascii_lowercase().ends_with(".svg")
            {
                return None;
            }
            let size = el.attr("sizes").and_then(|s| {
                s.split_ascii_whitespace()
                    .filter_map(|t| {
                        let (w, h) = t
                            .to_ascii_lowercase()
                            .split_once('x')
                            .map(|(w, h)| (w.parse::<u32>().ok(), h.parse::<u32>().ok()))?;
                        Some(w?.max(h?))
                    })
                    .min_by_key(|d| d.abs_diff(ICON_TARGET))
            });
            Some(IconCandidate { url, size, mime })
        })
        .collect()
}

/// Picks the candidate closest to 48 px; unsized entries rank last, document order breaks ties.
pub fn choose_icon(candidates: &[IconCandidate]) -> Option<Url> {
    candidates
        .iter()
        .enumerate()
        .min_by_key(|(i, c)| (c.size.map_or(u32::MAX, |s| s.abs_diff(ICON_TARGET)), *i))
        .map(|(_, c)| c.url.clone())
}

fn sniff_image(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if bytes.starts_with(&[0, 0, 1, 0]) {
        Some("image/x-icon")
    } else if bytes.starts_with(b"GIF8") {
        Some("image/gif")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

async fn download_icon(client: &reqwest::Client, url: Url) -> Option<Icon> {
    let mut response = client.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    if response
        .content_length()
        .is_some_and(|n| n > ICON_MAX as u64)
    {
        return None;
    }
    let header = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        })
        .unwrap_or_default();
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if bytes.len() + chunk.len() > ICON_MAX {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.is_empty() {
        return None;
    }
    let mime = if header.starts_with("image/") {
        header
    } else {
        sniff_image(&bytes)?.to_string()
    };
    Some(Icon { bytes, mime })
}

/// Discovers and downloads the icon of a site origin. Every failure yields `None`.
pub async fn fetch_favicon(origin: &str, limits: FetchLimits) -> Option<Icon> {
    let origin = Url::parse(origin).ok()?;
    let client = client(limits).ok()?;
    let root = origin.join("/").ok()?;
    if let Ok((html, base)) = fetch_html(&client, root.as_str(), limits).await
        && let Some(url) = choose_icon(&extract_icons(&html, &base))
        && let Some(icon) = download_icon(&client, url).await
    {
        return Some(icon);
    }
    download_icon(&client, origin.join("/favicon.ico").ok()?).await
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
        serve_routes(vec![("*", response)], delay).await
    }

    /// Answers each request by path; `*` matches any path, unknown paths get 404.
    async fn serve_routes(routes: Vec<(&'static str, Vec<u8>)>, delay: Duration) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let routes = std::sync::Arc::new(routes);
        tokio::spawn(async move {
            while let Ok((mut sock, _)) = listener.accept().await {
                let routes = routes.clone();
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]);
                    let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
                    let response = routes
                        .iter()
                        .find(|(p, _)| *p == "*" || *p == path)
                        .map(|(_, r)| r.clone())
                        .unwrap_or_else(not_found);
                    tokio::time::sleep(delay).await;
                    let _ = sock.write_all(&response).await;
                    let _ = sock.shutdown().await;
                });
            }
        });
        format!("http://{addr}/")
    }

    fn not_found() -> Vec<u8> {
        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()
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

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nrest";
    const ICO: &[u8] = b"\x00\x00\x01\x00rest";

    fn base() -> Url {
        Url::parse("https://www.a.example/home/").unwrap()
    }

    #[test]
    fn extract_icons_sized_pick() {
        let html = r#"<link rel="icon" sizes="16x16" href="/16.png">
            <link rel="icon" sizes="32x32" href="/32.png">
            <link rel="icon" sizes="192x192" href="/192.png">"#;
        let chosen = choose_icon(&extract_icons(html, &base())).unwrap();
        assert_eq!(chosen.as_str(), "https://www.a.example/32.png");
    }

    #[test]
    fn extract_icons_shortcut_and_apple() {
        let html = r#"<link rel="Shortcut Icon" href="/fav.ico">
            <link rel="apple-touch-icon" href="/apple.png">
            <link rel="stylesheet" href="/s.css">"#;
        let icons = extract_icons(html, &base());
        assert_eq!(icons.len(), 2);
        assert_eq!(icons[0].url.as_str(), "https://www.a.example/fav.ico");
        assert_eq!(icons[1].url.as_str(), "https://www.a.example/apple.png");
        // Unsized: document order.
        assert_eq!(choose_icon(&icons), Some(icons[0].url.clone()));
    }

    #[test]
    fn extract_icons_unsized_last() {
        let html = r#"<link rel="icon" href="/plain.ico">
            <link rel="apple-touch-icon" sizes="180x180" href="/apple.png">"#;
        let chosen = choose_icon(&extract_icons(html, &base())).unwrap();
        assert_eq!(chosen.path(), "/apple.png");
    }

    #[test]
    fn extract_icons_svg_skipped() {
        let html = r#"<link rel="icon" href="/icon.svg">
            <link rel="icon" type="image/svg+xml" href="/icon">"#;
        assert!(extract_icons(html, &base()).is_empty());
    }

    #[test]
    fn extract_icons_relative_against_redirected_base() {
        let html = r#"<link rel="icon" href="img/fav.png">"#;
        let chosen = choose_icon(&extract_icons(html, &base())).unwrap();
        assert_eq!(chosen.as_str(), "https://www.a.example/home/img/fav.png");
    }

    #[test]
    fn extract_icons_none() {
        let icons = extract_icons("<html><head><title>x</title></head></html>", &base());
        assert!(icons.is_empty());
        assert_eq!(choose_icon(&icons), None);
    }

    #[tokio::test]
    async fn favicon_link_found() {
        let url = serve_routes(
            vec![
                (
                    "/",
                    http(
                        "text/html",
                        br#"<link rel="icon" sizes="32x32" href="/i/32.png">"#,
                    ),
                ),
                ("/i/32.png", http("image/png", PNG)),
                ("/favicon.ico", http("image/x-icon", ICO)),
            ],
            Duration::ZERO,
        )
        .await;
        let icon = fetch_favicon(&url, FetchLimits::default()).await.unwrap();
        assert_eq!(icon.mime, "image/png");
        assert_eq!(icon.bytes, PNG);
    }

    #[tokio::test]
    async fn favicon_fallback_ico() {
        let url = serve_routes(
            vec![
                (
                    "/",
                    http("text/html", br#"<link rel="icon" href="/icon.svg">"#),
                ),
                ("/favicon.ico", http("image/x-icon", ICO)),
            ],
            Duration::ZERO,
        )
        .await;
        let icon = fetch_favicon(&url, FetchLimits::default()).await.unwrap();
        assert_eq!(icon.mime, "image/x-icon");
        assert_eq!(icon.bytes, ICO);
    }

    #[tokio::test]
    async fn favicon_sniffed_without_image_type() {
        let url = serve_routes(
            vec![("/favicon.ico", http("application/octet-stream", ICO))],
            Duration::ZERO,
        )
        .await;
        let icon = fetch_favicon(&url, FetchLimits::default()).await.unwrap();
        assert_eq!(icon.mime, "image/x-icon");
    }

    #[tokio::test]
    async fn favicon_non_image_rejected() {
        let url = serve_routes(
            vec![
                (
                    "/",
                    http("text/html", br#"<link rel="icon" href="/fav.png">"#),
                ),
                ("/fav.png", http("text/html", b"<html>nope</html>")),
                ("/favicon.ico", http("text/html", b"<html>nope</html>")),
            ],
            Duration::ZERO,
        )
        .await;
        assert_eq!(fetch_favicon(&url, FetchLimits::default()).await, None);
    }

    #[tokio::test]
    async fn favicon_oversized_rejected() {
        let mut big = PNG.to_vec();
        big.extend(std::iter::repeat_n(0u8, ICON_MAX + 1));
        let url = serve_routes(
            vec![("/favicon.ico", http("image/png", &big))],
            Duration::ZERO,
        )
        .await;
        assert_eq!(fetch_favicon(&url, FetchLimits::default()).await, None);
    }

    #[tokio::test]
    async fn favicon_unreachable_none() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let limits = FetchLimits {
            timeout: Duration::from_secs(2),
            ..FetchLimits::default()
        };
        assert_eq!(fetch_favicon(&format!("http://{addr}"), limits).await, None);
    }
}
