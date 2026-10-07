//! Output formats for bookmarks.

use std::fmt::Write as _;

use wobook_core::model::Bookmark;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    Tsv,
    Json,
    Jsonl,
    Pretty,
}

impl OutputFormat {
    /// `pretty` on a TTY, `tsv` otherwise.
    pub fn resolve(choice: Option<Self>) -> Self {
        use std::io::IsTerminal;
        choice.unwrap_or(if std::io::stdout().is_terminal() {
            Self::Pretty
        } else {
            Self::Tsv
        })
    }
}

fn clean(field: &str) -> String {
    field.replace(['\t', '\n', '\r'], " ")
}

pub fn tsv_line(b: &Bookmark) -> String {
    format!(
        "{}\t{}\t{}",
        clean(&b.url),
        clean(&b.title),
        clean(&b.tags.join(","))
    )
}

pub fn pretty(b: &Bookmark) -> String {
    let mut out = String::new();
    let title = if b.title.is_empty() {
        "(untitled)"
    } else {
        b.title.as_str()
    };
    let _ = writeln!(out, "{title}{}", if b.deleted { " [deleted]" } else { "" });
    let _ = writeln!(out, "   > {}", b.url);
    if !b.description.is_empty() {
        let _ = writeln!(out, "   + {}", b.description.replace('\n', "\n     "));
    }
    if !b.tags.is_empty() {
        let _ = writeln!(out, "   # {}", b.tags.join(", "));
    }
    out
}

pub fn render(bookmarks: &[Bookmark], format: OutputFormat) -> String {
    match format {
        OutputFormat::Tsv => bookmarks.iter().map(|b| tsv_line(b) + "\n").collect(),
        OutputFormat::Json => {
            let values: Vec<_> = bookmarks.iter().map(Bookmark::to_export_json).collect();
            serde_json::to_string_pretty(&values).unwrap_or_default() + "\n"
        }
        OutputFormat::Jsonl => bookmarks
            .iter()
            .map(|b| serde_json::to_string(b).unwrap_or_default() + "\n")
            .collect(),
        OutputFormat::Pretty => bookmarks.iter().map(pretty).collect::<Vec<_>>().join("\n"),
    }
}
