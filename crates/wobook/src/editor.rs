//! buku-style editor template.

use wobook_core::model::Bookmark;

const URL_MARK: &str = "# Add URL in next line (single line). Changing it MOVES the bookmark.";
const TITLE_MARK: &str =
    "# Add TITLE in next line (single line). Leave blank to web fetch, \"-\" for no title.";
const TAGS_MARK: &str = "# Add comma-separated TAGS in next line (single line).";
const DESC_MARK: &str = "# Add COMMENTS in next line(s). \"-\" for no comments.";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Edited {
    pub url: String,
    /// `None` when blank (fetch), `Some("")` for `-`.
    pub title: Option<String>,
    pub tags: Vec<String>,
    pub description: String,
}

pub fn render(b: Option<&Bookmark>) -> String {
    let (url, title, tags, desc) = match b {
        Some(b) => (
            b.url.as_str(),
            b.title.as_str(),
            b.tags.join(", "),
            b.description.as_str(),
        ),
        None => ("", "", String::new(), ""),
    };
    format!(
        "# Lines beginning with \"#\" will be stripped.\n{URL_MARK}\n{url}\n{TITLE_MARK}\n{title}\n{TAGS_MARK}\n{tags}\n{DESC_MARK}\n{desc}\n"
    )
}

#[derive(PartialEq)]
enum Section {
    None,
    Url,
    Title,
    Tags,
    Desc,
}

pub fn parse(text: &str) -> Edited {
    let mut section = Section::None;
    let (mut url, mut title, mut tags, mut desc) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for line in text.lines() {
        if line.starts_with('#') {
            section = if line.starts_with("# Add URL") {
                Section::Url
            } else if line.starts_with("# Add TITLE") {
                Section::Title
            } else if line.starts_with("# Add comma-separated TAGS") {
                Section::Tags
            } else if line.starts_with("# Add COMMENTS") {
                Section::Desc
            } else {
                section
            };
            continue;
        }
        match section {
            Section::Url => url.push(line),
            Section::Title => title.push(line),
            Section::Tags => tags.push(line),
            Section::Desc => desc.push(line),
            Section::None => {}
        }
    }
    let title = title.join(" ").trim().to_string();
    let description = desc.join("\n").trim().to_string();
    Edited {
        url: url.join("").trim().to_string(),
        title: match title.as_str() {
            "" => None,
            "-" => Some(String::new()),
            _ => Some(title),
        },
        tags: wobook_core::tags::parse(&tags.join(","))
            .into_iter()
            .map(|t| t.as_str().to_string())
            .collect(),
        description: if description == "-" {
            String::new()
        } else {
            description
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let b = Bookmark {
            url: "https://a.example/".into(),
            title: "T".into(),
            description: "line1\nline2".into(),
            tags: vec!["ui library".into(), "x".into()],
            ..Default::default()
        };
        let e = parse(&render(Some(&b)));
        assert_eq!(e.url, b.url);
        assert_eq!(e.title.as_deref(), Some("T"));
        assert_eq!(e.tags, b.tags);
        assert_eq!(e.description, b.description);
    }

    #[test]
    fn blank_and_dash_title() {
        let mut text = render(None);
        text = text.replacen("\n\n", "\nhttps://n.example/\n", 1);
        let e = parse(&text);
        assert_eq!(e.url, "https://n.example/");
        assert_eq!(e.title, None);
        let e = parse(&text.replace(&format!("{TITLE_MARK}\n"), &format!("{TITLE_MARK}\n-")));
        assert_eq!(e.title.as_deref(), Some(""));
    }
}
