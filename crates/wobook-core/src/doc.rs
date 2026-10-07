//! Automerge document layout (D4). The only module that knows the layout.
//!
//! ```text
//! ROOT.bookmarks[url] = Map { url, title, description, tags: Map<tag, true>,
//!                             created_ms, updated_ms, deleted }
//! ROOT.meta = Map { schema: 1 }
//! ```
//! Concurrent creation of the same URL yields conflicting maps under one key;
//! readers merge them (tags unioned, winner's scalars, empty fields filled) and
//! the next local write consolidates them into one map.

use automerge::{
    AutomergeError, ObjId, ObjType, ROOT, ReadDoc, ScalarValue, Value, transaction::Transactable,
};

use crate::model::Bookmark;

pub const SCHEMA: i64 = 1;
const BOOKMARKS: &str = "bookmarks";
const META: &str = "meta";

#[derive(Debug, thiserror::Error)]
pub enum DocError {
    #[error(transparent)]
    Automerge(#[from] AutomergeError),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("document has no bookmarks map; schema missing")]
    NoSchema,
}

type Result<T> = std::result::Result<T, DocError>;

fn map_child<D: ReadDoc>(doc: &D, obj: &ObjId, key: &str) -> Result<Option<ObjId>> {
    Ok(match doc.get(obj, key)? {
        Some((Value::Object(ObjType::Map), id)) => Some(id),
        _ => None,
    })
}

fn bookmarks_obj<D: ReadDoc>(doc: &D) -> Result<Option<ObjId>> {
    map_child(doc, &ROOT, BOOKMARKS)
}

/// Creates `ROOT.bookmarks` and `ROOT.meta.schema` when absent.
pub fn ensure_schema<T: Transactable>(tx: &mut T) -> Result<()> {
    if bookmarks_obj(tx)?.is_none() {
        tx.put_object(ROOT, BOOKMARKS, ObjType::Map)?;
    }
    let meta = match map_child(tx, &ROOT, META)? {
        Some(meta) => meta,
        None => tx.put_object(ROOT, META, ObjType::Map)?,
    };
    if tx.get(&meta, "schema")?.is_none() {
        tx.put(&meta, "schema", SCHEMA)?;
    }
    Ok(())
}

fn get_str<D: ReadDoc>(doc: &D, obj: &ObjId, key: &str) -> Result<String> {
    Ok(match doc.get(obj, key)? {
        Some((Value::Scalar(s), _)) => match s.as_ref() {
            ScalarValue::Str(text) => text.to_string(),
            _ => String::new(),
        },
        _ => String::new(),
    })
}

fn get_i64<D: ReadDoc>(doc: &D, obj: &ObjId, key: &str) -> Result<i64> {
    Ok(match doc.get(obj, key)? {
        Some((Value::Scalar(s), _)) => match s.as_ref() {
            ScalarValue::Int(n) => *n,
            ScalarValue::Uint(n) => i64::try_from(*n).unwrap_or(i64::MAX),
            ScalarValue::Timestamp(n) => *n,
            _ => 0,
        },
        _ => 0,
    })
}

fn get_bool<D: ReadDoc>(doc: &D, obj: &ObjId, key: &str) -> Result<bool> {
    Ok(matches!(
        doc.get(obj, key)?,
        Some((Value::Scalar(s), _)) if matches!(s.as_ref(), ScalarValue::Boolean(true))
    ))
}

fn read_tags<D: ReadDoc>(doc: &D, obj: &ObjId) -> Result<Vec<String>> {
    let Some(tags) = map_child(doc, obj, "tags")? else {
        return Ok(Vec::new());
    };
    Ok(doc.keys(&tags).collect())
}

fn read_obj<D: ReadDoc>(doc: &D, key: &str, obj: &ObjId) -> Result<Bookmark> {
    let mut url = get_str(doc, obj, "url")?;
    if url.is_empty() {
        url = key.to_string();
    }
    Ok(Bookmark {
        url,
        title: get_str(doc, obj, "title")?,
        description: get_str(doc, obj, "description")?,
        tags: read_tags(doc, obj)?,
        created_ms: get_i64(doc, obj, "created_ms")?,
        updated_ms: get_i64(doc, obj, "updated_ms")?,
        deleted: get_bool(doc, obj, "deleted")?,
    })
}

/// All conflicting maps stored under `bookmarks[key]`, winner first.
fn entries<D: ReadDoc>(doc: &D, bookmarks: &ObjId, key: &str) -> Result<Vec<ObjId>> {
    let mut out = Vec::new();
    if let Some((Value::Object(ObjType::Map), winner)) = doc.get(bookmarks, key)? {
        out.push(winner);
    }
    for (value, id) in doc.get_all(bookmarks, key)? {
        if matches!(value, Value::Object(ObjType::Map)) && !out.contains(&id) {
            out.push(id);
        }
    }
    Ok(out)
}

fn merged_read<D: ReadDoc>(doc: &D, bookmarks: &ObjId, key: &str) -> Result<Option<Bookmark>> {
    let mut result: Option<Bookmark> = None;
    for obj in entries(doc, bookmarks, key)? {
        let next = read_obj(doc, key, &obj)?;
        result = Some(match result {
            None => next,
            Some(mut acc) => {
                merge_into(&mut acc, &next);
                acc.created_ms = acc.created_ms.min(next.created_ms);
                acc
            }
        });
    }
    Ok(result.map(|mut b| {
        b.tags.sort();
        b.tags.dedup();
        b
    }))
}

/// Unions tags and fills empty title/description of `target` from `source`.
pub fn merge_into(target: &mut Bookmark, source: &Bookmark) {
    for tag in &source.tags {
        if !target.tags.contains(tag) {
            target.tags.push(tag.clone());
        }
    }
    target.tags.sort();
    if target.title.is_empty() {
        target.title.clone_from(&source.title);
    }
    if target.description.is_empty() {
        target.description.clone_from(&source.description);
    }
}

pub fn read<D: ReadDoc>(doc: &D, key: &str) -> Result<Option<Bookmark>> {
    let Some(bookmarks) = bookmarks_obj(doc)? else {
        return Ok(None);
    };
    merged_read(doc, &bookmarks, key)
}

pub fn read_all<D: ReadDoc>(doc: &D) -> Result<Vec<Bookmark>> {
    let Some(bookmarks) = bookmarks_obj(doc)? else {
        return Ok(Vec::new());
    };
    let keys: Vec<String> = doc.keys(&bookmarks).collect();
    let mut out = Vec::with_capacity(keys.len());
    for key in keys {
        if let Some(b) = merged_read(doc, &bookmarks, &key)? {
            out.push(b);
        }
    }
    Ok(out)
}

fn write_full<T: Transactable>(tx: &mut T, bookmarks: &ObjId, b: &Bookmark) -> Result<ObjId> {
    let obj = tx.put_object(bookmarks, b.url.as_str(), ObjType::Map)?;
    tx.put(&obj, "url", b.url.as_str())?;
    tx.put(&obj, "title", b.title.as_str())?;
    tx.put(&obj, "description", b.description.as_str())?;
    let tags = tx.put_object(&obj, "tags", ObjType::Map)?;
    for tag in &b.tags {
        tx.put(&tags, tag.as_str(), true)?;
    }
    tx.put(&obj, "created_ms", b.created_ms)?;
    tx.put(&obj, "updated_ms", b.updated_ms)?;
    tx.put(&obj, "deleted", b.deleted)?;
    Ok(obj)
}

/// Returns the single map for `key`, consolidating concurrent duplicates.
fn entry<T: Transactable>(tx: &mut T, key: &str) -> Result<Option<ObjId>> {
    let bookmarks = bookmarks_obj(tx)?.ok_or(DocError::NoSchema)?;
    let objs = entries(tx, &bookmarks, key)?;
    match objs.len() {
        0 => Ok(None),
        1 => Ok(objs.into_iter().next()),
        _ => {
            let merged = merged_read(tx, &bookmarks, key)?.ok_or(DocError::NoSchema)?;
            Ok(Some(write_full(tx, &bookmarks, &merged)?))
        }
    }
}

fn require<T: Transactable>(tx: &mut T, key: &str) -> Result<ObjId> {
    entry(tx, key)?.ok_or_else(|| DocError::NotFound(key.to_string()))
}

fn tags_obj<T: Transactable>(tx: &mut T, obj: &ObjId) -> Result<ObjId> {
    match map_child(tx, obj, "tags")? {
        Some(tags) => Ok(tags),
        None => Ok(tx.put_object(obj, "tags", ObjType::Map)?),
    }
}

/// Creates the bookmark or overwrites its scalar fields; tags are added
/// (never removed) so a concurrent add of the same URL unions them.
pub fn upsert<T: Transactable>(tx: &mut T, b: &Bookmark) -> Result<()> {
    let bookmarks = bookmarks_obj(tx)?.ok_or(DocError::NoSchema)?;
    let Some(obj) = entry(tx, &b.url)? else {
        write_full(tx, &bookmarks, b)?;
        return Ok(());
    };
    tx.put(&obj, "url", b.url.as_str())?;
    tx.put(&obj, "title", b.title.as_str())?;
    tx.put(&obj, "description", b.description.as_str())?;
    tx.put(&obj, "updated_ms", b.updated_ms)?;
    tx.put(&obj, "deleted", b.deleted)?;
    if get_i64(tx, &obj, "created_ms")? == 0 {
        tx.put(&obj, "created_ms", b.created_ms)?;
    }
    let tags = tags_obj(tx, &obj)?;
    for tag in &b.tags {
        put_tag(tx, &tags, tag)?;
    }
    Ok(())
}

/// Sets title and/or description.
pub fn set_fields<T: Transactable>(
    tx: &mut T,
    key: &str,
    title: Option<&str>,
    description: Option<&str>,
    now_ms: i64,
) -> Result<()> {
    let obj = require(tx, key)?;
    if let Some(title) = title {
        tx.put(&obj, "title", title)?;
    }
    if let Some(description) = description {
        tx.put(&obj, "description", description)?;
    }
    tx.put(&obj, "updated_ms", now_ms)?;
    Ok(())
}

pub fn set_tags<T: Transactable>(
    tx: &mut T,
    key: &str,
    tags: &[String],
    now_ms: i64,
) -> Result<()> {
    let obj = require(tx, key)?;
    let tags_id = tags_obj(tx, &obj)?;
    let current: Vec<String> = tx.keys(&tags_id).collect();
    for tag in current.iter().filter(|t| !tags.contains(t)) {
        tx.delete(&tags_id, tag.as_str())?;
    }
    for tag in tags.iter().filter(|t| !current.contains(t)) {
        tx.put(&tags_id, tag.as_str(), true)?;
    }
    tx.put(&obj, "updated_ms", now_ms)?;
    Ok(())
}

pub fn add_tags<T: Transactable>(
    tx: &mut T,
    key: &str,
    tags: &[String],
    now_ms: i64,
) -> Result<()> {
    let obj = require(tx, key)?;
    let tags_id = tags_obj(tx, &obj)?;
    for tag in tags {
        put_tag(tx, &tags_id, tag)?;
    }
    tx.put(&obj, "updated_ms", now_ms)?;
    Ok(())
}

/// Writes a fresh op for `tag` even when already present, so this add wins
/// over a concurrent remove (Automerge drops redundant puts).
fn put_tag<T: Transactable>(tx: &mut T, tags_id: &ObjId, tag: &str) -> Result<()> {
    if tx.get(tags_id, tag)?.is_some() {
        tx.delete(tags_id, tag)?;
    }
    tx.put(tags_id, tag, true)?;
    Ok(())
}

pub fn remove_tags<T: Transactable>(
    tx: &mut T,
    key: &str,
    tags: &[String],
    now_ms: i64,
) -> Result<()> {
    let obj = require(tx, key)?;
    let tags_id = tags_obj(tx, &obj)?;
    for tag in tags {
        if tx.get(&tags_id, tag.as_str())?.is_some() {
            tx.delete(&tags_id, tag.as_str())?;
        }
    }
    tx.put(&obj, "updated_ms", now_ms)?;
    Ok(())
}

pub fn tombstone<T: Transactable>(tx: &mut T, key: &str, now_ms: i64) -> Result<()> {
    let obj = require(tx, key)?;
    tx.put(&obj, "deleted", true)?;
    tx.put(&obj, "updated_ms", now_ms)?;
    Ok(())
}

pub fn restore<T: Transactable>(tx: &mut T, key: &str, now_ms: i64) -> Result<()> {
    let obj = require(tx, key)?;
    tx.put(&obj, "deleted", false)?;
    tx.put(&obj, "updated_ms", now_ms)?;
    Ok(())
}

/// Moves `old` to `new`: tombstones `old`; creates `new` with its fields, or
/// merges into an existing live `new` (tags union, empty fields filled).
/// Returns the resulting bookmark at `new`.
pub fn rename<T: Transactable>(tx: &mut T, old: &str, new: &str, now_ms: i64) -> Result<Bookmark> {
    let bookmarks = bookmarks_obj(tx)?.ok_or(DocError::NoSchema)?;
    let source = merged_read(tx, &bookmarks, old)?.ok_or_else(|| DocError::NotFound(old.into()))?;
    if old == new {
        return Ok(source);
    }
    let target = merged_read(tx, &bookmarks, new)?;
    let result = match target {
        Some(mut existing) if !existing.deleted => {
            merge_into(&mut existing, &source);
            existing.updated_ms = now_ms;
            existing
        }
        Some(mut existing) => {
            // Tombstoned target: revive it with the source's data.
            existing.title.clone_from(&source.title);
            existing.description.clone_from(&source.description);
            merge_into(&mut existing, &source);
            existing.created_ms = source.created_ms;
            existing.deleted = false;
            existing.updated_ms = now_ms;
            existing
        }
        None => Bookmark {
            url: new.to_string(),
            updated_ms: now_ms,
            deleted: false,
            ..source.clone()
        },
    };
    upsert(tx, &result)?;
    tombstone(tx, old, now_ms)?;
    Ok(result)
}

/// Sorted `v1:`-prefixed heads string used as projection checkpoint.
pub fn heads_string(doc: &automerge::Automerge) -> String {
    let mut heads: Vec<String> = doc.get_heads().iter().map(ToString::to_string).collect();
    heads.sort();
    format!("v1:{}", heads.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;
    use automerge::{AutoCommit, Automerge, transaction::CommitOptions};

    fn bm(url: &str, tags: &[&str]) -> Bookmark {
        Bookmark {
            url: url.into(),
            title: String::new(),
            description: String::new(),
            tags: tags.iter().map(ToString::to_string).collect(),
            created_ms: 1,
            updated_ms: 1,
            deleted: false,
        }
    }

    fn base() -> AutoCommit {
        let mut doc = AutoCommit::new();
        ensure_schema(&mut doc).unwrap();
        doc
    }

    fn fork(doc: &mut AutoCommit) -> AutoCommit {
        doc.fork().with_actor(automerge::ActorId::random())
    }

    #[test]
    fn upsert_and_read() {
        let mut doc = base();
        upsert(&mut doc, &bm("https://a/", &["x", "y"])).unwrap();
        let got = read(&doc, "https://a/").unwrap().unwrap();
        assert_eq!(got.tags, ["x", "y"]);
        assert!(!got.deleted);
        assert_eq!(read_all(&doc).unwrap().len(), 1);
    }

    #[test]
    fn concurrent_tag_add_vs_remove_add_wins() {
        let mut a = base();
        upsert(&mut a, &bm("https://a/", &["x"])).unwrap();
        let mut b = fork(&mut a);
        remove_tags(&mut a, "https://a/", &["x".into()], 2).unwrap();
        add_tags(&mut b, "https://a/", &["x".into()], 2).unwrap();
        a.merge(&mut b).unwrap();
        b.merge(&mut a).unwrap();
        for doc in [&a, &b] {
            assert_eq!(read(doc, "https://a/").unwrap().unwrap().tags, ["x"]);
        }
    }

    #[test]
    fn concurrent_same_url_add_unions_tags() {
        let mut a = base();
        let mut b = fork(&mut a);
        let mut one = bm("https://example.com/", &["a"]);
        one.title = "A".into();
        let mut two = bm("https://example.com/", &["b"]);
        two.title = "B".into();
        upsert(&mut a, &one).unwrap();
        upsert(&mut b, &two).unwrap();
        a.merge(&mut b).unwrap();
        b.merge(&mut a).unwrap();
        let ra = read_all(&a).unwrap();
        let rb = read_all(&b).unwrap();
        assert_eq!(ra.len(), 1);
        assert_eq!(ra, rb);
        assert_eq!(ra[0].tags, ["a", "b"]);
        // A later local write consolidates without losing tags.
        add_tags(&mut a, "https://example.com/", &["c".into()], 3).unwrap();
        assert_eq!(
            read(&a, "https://example.com/").unwrap().unwrap().tags,
            ["a", "b", "c"]
        );
    }

    #[test]
    fn concurrent_title_edits_converge() {
        let mut a = base();
        upsert(&mut a, &bm("https://a/", &[])).unwrap();
        let mut b = fork(&mut a);
        set_fields(&mut a, "https://a/", Some("one"), None, 2).unwrap();
        set_fields(&mut b, "https://a/", Some("two"), None, 2).unwrap();
        a.merge(&mut b).unwrap();
        b.merge(&mut a).unwrap();
        assert_eq!(
            read(&a, "https://a/").unwrap().unwrap().title,
            read(&b, "https://a/").unwrap().unwrap().title
        );
    }

    #[test]
    fn delete_vs_concurrent_edit() {
        let mut a = base();
        upsert(&mut a, &bm("https://a/", &[])).unwrap();
        let mut b = fork(&mut a);
        tombstone(&mut a, "https://a/", 2).unwrap();
        set_fields(&mut b, "https://a/", None, Some("edited"), 2).unwrap();
        a.merge(&mut b).unwrap();
        b.merge(&mut a).unwrap();
        for doc in [&a, &b] {
            let got = read(doc, "https://a/").unwrap().unwrap();
            assert!(got.deleted);
            assert_eq!(got.description, "edited");
        }
    }

    #[test]
    fn rename_keeps_metadata() {
        let mut doc = base();
        let mut src = bm("https://a.example/", &["x"]);
        src.title = "T".into();
        src.created_ms = 42;
        upsert(&mut doc, &src).unwrap();
        rename(&mut doc, "https://a.example/", "https://b.example/", 5).unwrap();
        assert!(read(&doc, "https://a.example/").unwrap().unwrap().deleted);
        let got = read(&doc, "https://b.example/").unwrap().unwrap();
        assert_eq!((got.title.as_str(), got.created_ms), ("T", 42));
        assert_eq!(got.tags, ["x"]);
    }

    #[test]
    fn rename_onto_existing_merges() {
        let mut doc = base();
        let mut src = bm("https://a/", &["x"]);
        src.description = "from a".into();
        upsert(&mut doc, &src).unwrap();
        let mut dst = bm("https://b/", &["y"]);
        dst.title = "B".into();
        upsert(&mut doc, &dst).unwrap();
        rename(&mut doc, "https://a/", "https://b/", 5).unwrap();
        let got = read(&doc, "https://b/").unwrap().unwrap();
        assert_eq!(got.tags, ["x", "y"]);
        assert_eq!(
            (got.title.as_str(), got.description.as_str()),
            ("B", "from a")
        );
        assert!(read(&doc, "https://a/").unwrap().unwrap().deleted);
    }

    #[test]
    fn works_with_plain_automerge_transactions() {
        let mut doc = Automerge::new();
        doc.transact_with::<_, _, DocError, _>(
            |_| CommitOptions::default(),
            |tx| {
                ensure_schema(tx)?;
                upsert(tx, &bm("https://a/", &["x"]))
            },
        )
        .unwrap();
        assert_eq!(read_all(&doc).unwrap().len(), 1);
        assert!(heads_string(&doc).starts_with("v1:"));
    }
}
