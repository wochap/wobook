//! Import/export formats (D10): JSONL, Netscape bookmarks HTML, buku SQLite.

use std::path::Path;

use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};

use crate::model::{Bookmark, Record};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Jsonl,
    Netscape,
    Buku,
}

impl Format {
    /// `.jsonl`/`.json` → JSONL, `.html`/`.htm` → Netscape, `.db`/`.sqlite` → buku.
    pub fn infer(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "jsonl" | "json" | "ndjson" => Some(Self::Jsonl),
            "html" | "htm" => Some(Self::Netscape),
            "db" | "sqlite" | "sqlite3" => Some(Self::Buku),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineError {
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportReport {
    pub added: usize,
    pub merged: usize,
    pub skipped: usize,
    pub errors: Vec<LineError>,
}

/// Parsed records plus per-line errors.
#[derive(Debug, Default)]
pub struct Parsed {
    pub records: Vec<(usize, Record)>,
    pub errors: Vec<LineError>,
}

#[derive(Debug, thiserror::Error)]
pub enum InterchangeError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

pub fn read_jsonl(text: &str) -> Parsed {
    let mut out = Parsed::default();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Record>(line) {
            Ok(record) => out.records.push((i + 1, record)),
            Err(e) => out.errors.push(LineError {
                line: i + 1,
                message: e.to_string(),
            }),
        }
    }
    out
}

pub fn write_jsonl(bookmarks: &[Bookmark]) -> String {
    let mut out = String::new();
    for b in bookmarks {
        out.push_str(&b.to_export_json().to_string());
        out.push('\n');
    }
    out
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn write_netscape(bookmarks: &[Bookmark]) -> String {
    let mut out = String::from(
        "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n<TITLE>Bookmarks</TITLE>\n<H1>Bookmarks</H1>\n<DL><p>\n",
    );
    for b in bookmarks {
        out.push_str(&format!(
            "    <DT><A HREF=\"{}\" ADD_DATE=\"{}\" LAST_MODIFIED=\"{}\" TAGS=\"{}\">{}</A>\n",
            escape(&b.url),
            b.created_ms / 1000,
            b.updated_ms / 1000,
            escape(&b.tags.join(",")),
            escape(&b.title)
        ));
        if !b.description.is_empty() {
            out.push_str(&format!("    <DD>{}\n", escape(&b.description)));
        }
    }
    out.push_str("</DL><p>\n");
    out
}

fn following_dd(anchor: &ElementRef<'_>) -> Option<String> {
    // html5ever closes <DT> at <DD>, so the description is the DT's next
    // element sibling (or, in some exports, the anchor's).
    let start = anchor
        .parent()
        .and_then(ElementRef::wrap)
        .filter(|p| p.value().name() == "dt")
        .unwrap_or(*anchor);
    let next = start.next_siblings().find_map(ElementRef::wrap)?;
    if next.value().name() != "dd" {
        return None;
    }
    // Only the DD's own text, not nested lists.
    let text: String = next
        .children()
        .filter_map(|c| c.value().as_text().map(|t| t.to_string()))
        .collect();
    let text = crate::fetch::sanitize(&text, usize::MAX);
    (!text.is_empty()).then_some(text)
}

pub fn read_netscape(html: &str) -> Parsed {
    let doc = Html::parse_document(html);
    let mut out = Parsed::default();
    let Ok(sel) = Selector::parse("a[href]") else {
        return out;
    };
    for (i, a) in doc.select(&sel).enumerate() {
        let attr = |name: &str| a.value().attr(name).map(str::to_string);
        let Some(url) = attr("href") else { continue };
        let created_ms = attr("add_date")
            .and_then(|s| s.trim().parse::<i64>().ok())
            .map(|s| s * 1000);
        let tags = attr("tags")
            .map(|t| {
                crate::tags::parse(&t)
                    .into_iter()
                    .map(|t| t.as_str().to_string())
                    .collect()
            })
            .unwrap_or_default();
        let title = crate::fetch::sanitize(&a.text().collect::<String>(), usize::MAX);
        out.records.push((
            i + 1,
            Record {
                url,
                title: Some(title),
                description: following_dd(&a),
                tags,
                created_ms,
                ..Default::default()
            },
        ));
    }
    out
}

/// Reads a buku database read-only: `bookmarks(URL, metadata, tags, desc)`.
pub fn read_buku(path: &Path, now_ms: i64) -> Result<Parsed, InterchangeError> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let mut stmt = conn.prepare("SELECT URL, metadata, tags, desc FROM bookmarks ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, Option<String>>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<String>>(3)?,
        ))
    })?;
    let mut out = Parsed::default();
    for (i, row) in rows.enumerate() {
        match row {
            Ok((Some(url), title, tags, desc)) => out.records.push((
                i + 1,
                Record {
                    url,
                    title,
                    description: desc,
                    tags: crate::tags::parse(tags.as_deref().unwrap_or(""))
                        .into_iter()
                        .map(|t| t.as_str().to_string())
                        .collect(),
                    created_ms: Some(now_ms),
                    ..Default::default()
                },
            )),
            Ok((None, ..)) => out.errors.push(LineError {
                line: i + 1,
                message: "missing URL".into(),
            }),
            Err(e) => out.errors.push(LineError {
                line: i + 1,
                message: e.to_string(),
            }),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonl_bad_line_reported() {
        let parsed = read_jsonl("{\"url\":\"a.example\"}\n{\"url\":\"b.example\",\"x\":1}\nnope\n");
        assert_eq!(parsed.records.len(), 2);
        assert_eq!(parsed.errors[0].line, 3);
    }

    #[test]
    fn netscape_roundtrip_and_nested() {
        let b = Bookmark {
            url: "https://a.example/".into(),
            title: "A & B".into(),
            description: "desc".into(),
            tags: vec!["ui library".into(), "x".into()],
            created_ms: 5000,
            ..Default::default()
        };
        let html = write_netscape(&[b]);
        let parsed = read_netscape(&html);
        let r = &parsed.records[0].1;
        assert_eq!(r.title.as_deref(), Some("A & B"));
        assert_eq!(r.description.as_deref(), Some("desc"));
        assert_eq!(r.tags, ["ui library", "x"]);
        assert_eq!(r.created_ms, Some(5000));

        let nested = "<!DOCTYPE NETSCAPE-Bookmark-file-1><DL><p><DT><H3>Folder</H3><DL><p><DT><A HREF=\"https://n.example/\" ADD_DATE=\"1\">N</A><DD>nd</DL><p><DT><A HREF=\"https://m.example/\">M</A></DL>";
        let parsed = read_netscape(nested);
        assert_eq!(parsed.records.len(), 2);
        assert_eq!(parsed.records[0].1.description.as_deref(), Some("nd"));
    }

    #[test]
    fn infer_format() {
        assert_eq!(Format::infer(Path::new("x.db")), Some(Format::Buku));
        assert_eq!(Format::infer(Path::new("x.jsonl")), Some(Format::Jsonl));
        assert_eq!(Format::infer(Path::new("x.HTML")), Some(Format::Netscape));
    }
}
