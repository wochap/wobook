//! Disposable SQLite read model rebuilt from Automerge heads (D6).

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

use crate::model::Bookmark;

pub const SCHEMA_VERSION: &str = "1";

const SCHEMA: &str = "
CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT);
CREATE TABLE bookmarks(url TEXT PRIMARY KEY, title TEXT, description TEXT, created_ms INTEGER, updated_ms INTEGER, deleted INTEGER);
CREATE TABLE tags(url TEXT REFERENCES bookmarks(url) ON DELETE CASCADE, tag TEXT, PRIMARY KEY(url, tag));
CREATE INDEX tags_tag ON tags(tag);
";

#[derive(Debug, thiserror::Error)]
pub enum ProjectionError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Doc(#[from] crate::doc::DocError),
}

type Result<T> = std::result::Result<T, ProjectionError>;

pub struct ReadModel {
    conn: Connection,
    path: PathBuf,
}

fn try_open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    let has_meta: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='meta'",
            [],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if !has_meta {
        conn.execute_batch(SCHEMA)?;
        conn.execute(
            "INSERT INTO meta(key, value) VALUES('schema_version', ?1)",
            [SCHEMA_VERSION],
        )?;
        return Ok(conn);
    }
    let version: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key='schema_version'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if version.as_deref() != Some(SCHEMA_VERSION) {
        return Err(rusqlite::Error::InvalidQuery.into());
    }
    Ok(conn)
}

fn set_private(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

impl ReadModel {
    /// Opens the read model, discarding it when unreadable or of another schema.
    pub fn open_or_recreate(path: &Path) -> Result<Self> {
        let conn = match try_open(path) {
            Ok(conn) => conn,
            Err(_) => {
                for suffix in ["", "-wal", "-shm", "-journal"] {
                    let mut p = path.as_os_str().to_owned();
                    p.push(suffix);
                    let _ = std::fs::remove_file(PathBuf::from(p));
                }
                try_open(path)?
            }
        };
        set_private(path);
        Ok(Self {
            conn,
            path: path.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn checkpoint(&self) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key='heads_checkpoint'",
                [],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Rebuilds from `doc` unless its heads equal the checkpoint. Returns
    /// whether a rebuild happened.
    pub fn reconcile(&mut self, doc: &automerge::Automerge) -> Result<bool> {
        let heads = crate::doc::heads_string(doc);
        if self.checkpoint()?.as_deref() == Some(heads.as_str()) {
            return Ok(false);
        }
        let all = crate::doc::read_all(doc)?;
        self.rebuild(&all, &heads)?;
        Ok(true)
    }

    /// Replaces all rows in one transaction.
    pub fn rebuild(&mut self, all: &[Bookmark], heads: &str) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM tags", [])?;
        tx.execute("DELETE FROM bookmarks", [])?;
        {
            let mut ins = tx.prepare(
                "INSERT INTO bookmarks(url, title, description, created_ms, updated_ms, deleted) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            let mut tag = tx.prepare("INSERT OR IGNORE INTO tags(url, tag) VALUES(?1, ?2)")?;
            for b in all {
                ins.execute(params![
                    b.url,
                    b.title,
                    b.description,
                    b.created_ms,
                    b.updated_ms,
                    b.deleted
                ])?;
                for t in &b.tags {
                    tag.execute(params![b.url, t])?;
                }
            }
        }
        tx.execute(
            "INSERT INTO meta(key, value) VALUES('heads_checkpoint', ?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [heads],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn tags_of(&self, url: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT tag FROM tags WHERE url=?1 ORDER BY tag")?;
        let rows = stmt.query_map([url], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn get(&self, url: &str) -> Result<Option<Bookmark>> {
        let row = self
            .conn
            .query_row(
                "SELECT url, title, description, created_ms, updated_ms, deleted FROM bookmarks WHERE url=?1",
                [url],
                row_to_bookmark,
            )
            .optional()?;
        match row {
            Some(mut b) => {
                b.tags = self.tags_of(&b.url)?;
                Ok(Some(b))
            }
            None => Ok(None),
        }
    }

    /// Bookmarks carrying every tag in `tags`, newest `created_ms` first.
    pub fn list(&self, tags: &[String], include_deleted: bool) -> Result<Vec<Bookmark>> {
        let mut sql = String::from(
            "SELECT url, title, description, created_ms, updated_ms, deleted FROM bookmarks WHERE 1=1",
        );
        if !include_deleted {
            sql.push_str(" AND deleted=0");
        }
        for i in 0..tags.len() {
            sql.push_str(&format!(
                " AND url IN (SELECT url FROM tags WHERE tag=?{})",
                i + 1
            ));
        }
        sql.push_str(" ORDER BY created_ms DESC, url ASC");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(tags.iter()), row_to_bookmark)?;
        let mut out: Vec<Bookmark> = rows.collect::<rusqlite::Result<_>>()?;
        let mut tag_stmt = self
            .conn
            .prepare_cached("SELECT url, tag FROM tags ORDER BY tag")?;
        let mut map: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for row in
            tag_stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        {
            let (url, tag) = row?;
            map.entry(url).or_default().push(tag);
        }
        for b in &mut out {
            b.tags = map.remove(&b.url).unwrap_or_default();
        }
        Ok(out)
    }

    /// Tag counts over non-deleted bookmarks, sorted by tag.
    pub fn tags(&self) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.tag, COUNT(*) FROM tags t JOIN bookmarks b ON b.url=t.url WHERE b.deleted=0 GROUP BY t.tag ORDER BY t.tag",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn counts(&self) -> Result<(i64, i64)> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(SUM(deleted=0),0), COALESCE(SUM(deleted=1),0) FROM bookmarks",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?)
    }
}

fn row_to_bookmark(r: &rusqlite::Row<'_>) -> rusqlite::Result<Bookmark> {
    Ok(Bookmark {
        url: r.get(0)?,
        title: r.get(1)?,
        description: r.get(2)?,
        tags: Vec::new(),
        created_ms: r.get(3)?,
        updated_ms: r.get(4)?,
        deleted: r.get(5)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use automerge::{Automerge, transaction::CommitOptions};

    fn doc_with(n: usize) -> Automerge {
        let mut doc = Automerge::new();
        doc.transact_with::<_, _, crate::doc::DocError, _>(
            |_| CommitOptions::default(),
            |tx| {
                crate::doc::ensure_schema(tx)?;
                for i in 0..n {
                    crate::doc::upsert(
                        tx,
                        &Bookmark {
                            url: format!("https://e{i}.example/"),
                            title: format!("t{i}"),
                            tags: vec!["a".into()],
                            created_ms: i as i64,
                            ..Default::default()
                        },
                    )?;
                }
                Ok(())
            },
        )
        .unwrap();
        doc
    }

    #[test]
    fn reconcile_is_checkpointed() {
        let dir = tempfile::tempdir().unwrap();
        let mut rm = ReadModel::open_or_recreate(&dir.path().join("rm.sqlite")).unwrap();
        let doc = doc_with(3);
        assert!(rm.reconcile(&doc).unwrap());
        assert!(!rm.reconcile(&doc).unwrap());
        assert_eq!(rm.list(&[], false).unwrap().len(), 3);
        assert_eq!(
            rm.checkpoint().unwrap().unwrap(),
            crate::doc::heads_string(&doc)
        );
    }

    #[test]
    fn schema_mismatch_recreates() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rm.sqlite");
        {
            let rm = ReadModel::open_or_recreate(&path).unwrap();
            rm.conn
                .execute("UPDATE meta SET value='0' WHERE key='schema_version'", [])
                .unwrap();
        }
        let mut rm = ReadModel::open_or_recreate(&path).unwrap();
        assert!(rm.reconcile(&doc_with(1)).unwrap());
    }

    #[test]
    fn rebuild_5k_fast() {
        let dir = tempfile::tempdir().unwrap();
        let mut rm = ReadModel::open_or_recreate(&dir.path().join("rm.sqlite")).unwrap();
        let doc = doc_with(5000);
        let start = std::time::Instant::now();
        rm.reconcile(&doc).unwrap();
        assert!(start.elapsed() < std::time::Duration::from_secs(10));
        assert_eq!(rm.counts().unwrap(), (5000, 0));
    }
}
