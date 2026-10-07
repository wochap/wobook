//! Fuzzy search over the read model with nucleo (D7).

use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{CaseMatching, Normalization, Pattern},
};
use serde::{Deserialize, Serialize};

use crate::{model::Bookmark, projection::ReadModel};

/// Char-offset boundaries of each haystack segment: `[start, end)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segments {
    pub title: [u32; 2],
    pub url: [u32; 2],
    pub description: [u32; 2],
    pub tags: [u32; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hit {
    pub bookmark: Bookmark,
    pub score: u32,
    pub indices: Vec<u32>,
    pub segments: Segments,
}

/// Haystack `title\nurl\ndescription\ntags joined by space` and its segments.
pub fn haystack(b: &Bookmark) -> (String, Segments) {
    let tags = b.tags.join(" ");
    let len = |s: &str| u32::try_from(s.chars().count()).unwrap_or(u32::MAX);
    let mut pos = 0u32;
    let mut seg = |s: &str| {
        let start = pos;
        pos = start + len(s) + 1;
        [start, start + len(s)]
    };
    let segments = Segments {
        title: seg(&b.title),
        url: seg(&b.url),
        description: seg(&b.description),
        tags: seg(&tags),
    };
    (
        format!("{}\n{}\n{}\n{}", b.title, b.url, b.description, tags),
        segments,
    )
}

pub struct Searcher {
    matcher: Matcher,
}

impl Default for Searcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Searcher {
    pub fn new() -> Self {
        Self {
            matcher: Matcher::new(Config::DEFAULT),
        }
    }

    /// Ranks `candidates` against `query`. Empty query keeps the input order
    /// (callers pass `created_ms` desc) with zero scores.
    pub fn rank(
        &mut self,
        query: &str,
        candidates: Vec<Bookmark>,
        limit: Option<usize>,
    ) -> Vec<Hit> {
        let query = query.trim();
        let mut hits: Vec<Hit> = Vec::new();
        if query.is_empty() {
            hits = candidates
                .into_iter()
                .map(|bookmark| {
                    let (_, segments) = haystack(&bookmark);
                    Hit {
                        bookmark,
                        score: 0,
                        indices: Vec::new(),
                        segments,
                    }
                })
                .collect();
        } else {
            let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
            let mut buf = Vec::new();
            for bookmark in candidates {
                let (text, segments) = haystack(&bookmark);
                let mut indices = Vec::new();
                if let Some(score) = pattern.indices(
                    Utf32Str::new(&text, &mut buf),
                    &mut self.matcher,
                    &mut indices,
                ) {
                    indices.sort_unstable();
                    indices.dedup();
                    hits.push(Hit {
                        bookmark,
                        score,
                        indices,
                        segments,
                    });
                }
            }
            hits.sort_by(|a, b| {
                b.score
                    .cmp(&a.score)
                    .then(b.bookmark.updated_ms.cmp(&a.bookmark.updated_ms))
                    .then(a.bookmark.url.cmp(&b.bookmark.url))
            });
        }
        if let Some(limit) = limit {
            hits.truncate(limit);
        }
        hits
    }

    /// Tag filter in SQL, then fuzzy ranking in memory.
    pub fn search(
        &mut self,
        model: &ReadModel,
        query: &str,
        tags: &[String],
        include_deleted: bool,
        limit: Option<usize>,
    ) -> Result<Vec<Hit>, crate::projection::ProjectionError> {
        let candidates = model.list(tags, include_deleted)?;
        Ok(self.rank(query, candidates, limit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bm(url: &str, title: &str, tags: &[&str], updated: i64) -> Bookmark {
        Bookmark {
            url: url.into(),
            title: title.into(),
            tags: tags.iter().map(ToString::to_string).collect(),
            updated_ms: updated,
            ..Default::default()
        }
    }

    #[test]
    fn abbreviated_query_ranks_shadcn_first() {
        let items = vec![
            bm("https://example.com/", "Some Channel UI", &[], 3),
            bm("https://ui.shadcn.com/", "shadcn/ui", &["ui library"], 1),
            bm("https://vuejs.org/", "Vue", &["vue"], 2),
        ];
        let hits = Searcher::new().rank("shcn ui", items, None);
        assert_eq!(hits[0].bookmark.url, "https://ui.shadcn.com/");
        let (text, _) = haystack(&hits[0].bookmark);
        let n = text.chars().count() as u32;
        assert!(!hits[0].indices.is_empty());
        assert!(hits[0].indices.iter().all(|i| *i < n));
    }

    #[test]
    fn tag_only_match() {
        let hits = Searcher::new().rank(
            "systemd",
            vec![bm("https://x.example/", "X", &["systemd"], 1)],
            None,
        );
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn segments_cover_haystack() {
        let b = bm("https://a/", "Tí", &["x"], 1);
        let (text, seg) = haystack(&b);
        assert_eq!(seg.title, [0, 2]);
        assert_eq!(seg.url, [3, 13]);
        assert_eq!(seg.tags[1] as usize, text.chars().count());
    }
}
