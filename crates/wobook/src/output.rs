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

/// How `--color` was requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

impl ColorChoice {
    /// `auto` colours only on a terminal with `NO_COLOR` unset or empty.
    pub fn enabled(self) -> bool {
        use std::io::IsTerminal;
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => {
                std::io::stdout().is_terminal()
                    && std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty())
            }
        }
    }
}

/// SGR codes for `pretty`, buku's default scheme `oKlxm`; empty when off.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub title: &'static str,
    pub marker: &'static str,
    pub url: &'static str,
    pub tags: &'static str,
    pub reset: &'static str,
}

impl Palette {
    pub fn new(color: bool) -> Self {
        if color {
            Self {
                title: "\x1b[92;1m",
                marker: "\x1b[91m",
                url: "\x1b[93m",
                tags: "\x1b[94m",
                reset: "\x1b[0m",
            }
        } else {
            Self {
                title: "",
                marker: "",
                url: "",
                tags: "",
                reset: "",
            }
        }
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

pub fn pretty(b: &Bookmark, p: &Palette) -> String {
    let mut out = String::new();
    let title = if b.title.is_empty() {
        "(untitled)"
    } else {
        b.title.as_str()
    };
    let _ = writeln!(
        out,
        "{}{title}{}{}",
        p.title,
        p.reset,
        if b.deleted { " [deleted]" } else { "" }
    );
    let _ = writeln!(
        out,
        "   {}>{} {}{}{}",
        p.marker, p.reset, p.url, b.url, p.reset
    );
    if !b.description.is_empty() {
        let _ = writeln!(
            out,
            "   {}+{} {}",
            p.marker,
            p.reset,
            b.description.replace('\n', "\n     ")
        );
    }
    if !b.tags.is_empty() {
        let _ = writeln!(
            out,
            "   {}#{} {}{}{}",
            p.marker,
            p.reset,
            p.tags,
            b.tags.join(", "),
            p.reset
        );
    }
    out
}

pub fn render(bookmarks: &[Bookmark], format: OutputFormat, palette: &Palette) -> String {
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
        OutputFormat::Pretty => bookmarks
            .iter()
            .map(|b| pretty(b, palette))
            .collect::<Vec<_>>()
            .join("\n"),
    }
}
