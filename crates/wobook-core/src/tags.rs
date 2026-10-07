//! Tag normalization (D3).

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Tag(String);

impl Tag {
    /// Normalizes one tag; `None` when it is empty after normalization.
    pub fn new(input: &str) -> Option<Self> {
        let cleaned = input
            .replace(',', " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        (!cleaned.is_empty()).then_some(Self(cleaned))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Parses comma-separated tags, deduplicated in first-occurrence order.
pub fn parse(input: &str) -> Vec<Tag> {
    normalize_all(input.split(','))
}

/// Normalizes a list of individual tags, deduplicated in first-occurrence order.
pub fn normalize_all<'a>(items: impl IntoIterator<Item = &'a str>) -> Vec<Tag> {
    let mut out: Vec<Tag> = Vec::new();
    for tag in items.into_iter().filter_map(Tag::new) {
        if !out.contains(&tag) {
            out.push(tag);
        }
    }
    out
}

/// Sorted display form.
pub fn sorted(mut tags: Vec<Tag>) -> Vec<Tag> {
    tags.sort();
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strs(tags: &[Tag]) -> Vec<&str> {
        tags.iter().map(Tag::as_str).collect()
    }

    #[test]
    fn multi_word_tags_survive() {
        let tags = parse("UI Library, react ,  ai   agent,,react");
        assert_eq!(strs(&tags), ["ui library", "react", "ai agent"]);
    }

    #[test]
    fn commas_never_inside() {
        assert_eq!(Tag::new("a,b").unwrap().as_str(), "a b");
        assert!(Tag::new(" , ").is_none());
    }

    #[test]
    fn sorted_for_display() {
        assert_eq!(strs(&sorted(parse("b,a"))), ["a", "b"]);
    }
}
