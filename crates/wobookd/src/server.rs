//! Request dispatcher (D9).

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};

use automerge_repo::{DocHandle, Repo};
use serde_json::{Value, json};
use tokio::sync::{Mutex as AsyncMutex, Notify, mpsc};
use wobook_core::{
    doc,
    fetch::{self, FetchLimits},
    interchange::{self, Format, ImportReport},
    model::{Bookmark, Record},
    now_ms,
    projection::ReadModel,
    protocol::{ErrorCode, Request, Response},
    search::Searcher,
    tags,
};

use crate::hooks::{self, PostJob};

pub struct Daemon {
    /// Swapped when this device joins a mesh (D7).
    pub store: std::sync::RwLock<(Repo, DocHandle)>,
    pub joining: std::sync::atomic::AtomicBool,
    pub sync: Arc<wobook_sync::SyncEngine>,
    pub model: Mutex<ReadModel>,
    pub searcher: Mutex<Searcher>,
    pub writes: AsyncMutex<()>,
    pub data_dir: PathBuf,
    pub socket: PathBuf,
    pub hooks_dir: PathBuf,
    pub started: Instant,
    pub post: mpsc::UnboundedSender<PostJob>,
    pub shutdown: Notify,
}

pub struct Failure(pub ErrorCode, pub String);

type Result<T> = std::result::Result<T, Failure>;

fn internal(e: impl std::fmt::Display) -> Failure {
    Failure(ErrorCode::Internal, e.to_string())
}

fn not_found(url: &str) -> Failure {
    Failure(ErrorCode::NotFound, format!("not found: {url}"))
}

fn norm_url(input: &str) -> Result<String> {
    wobook_core::url::normalize(input)
        .map(|u| u.into_string())
        .map_err(|e| Failure(ErrorCode::InvalidUrl, e.to_string()))
}

fn norm_tags(input: &[String]) -> Vec<String> {
    tags::normalize_all(input.iter().map(String::as_str))
        .into_iter()
        .map(|t| t.as_str().to_string())
        .collect()
}

fn to_json(b: &Bookmark) -> Value {
    serde_json::to_value(b).unwrap_or(Value::Null)
}

fn change_err(e: doc::DocError) -> automerge_repo::Error {
    automerge_repo::Error::Change(e.to_string())
}

impl Daemon {
    pub fn doc(&self) -> DocHandle {
        self.store.read().expect("store lock").1.clone()
    }

    pub fn repo(&self) -> Repo {
        self.store.read().expect("store lock").0.clone()
    }

    /// Rebuilds the read model when the document heads moved.
    pub async fn reconcile(&self) -> Result<()> {
        let heads = self.doc().read(doc::heads_string).await.map_err(internal)?;
        let current = self
            .model
            .lock()
            .map_err(internal)?
            .checkpoint()
            .map_err(internal)?;
        if current.as_deref() == Some(heads.as_str()) {
            return Ok(());
        }
        let (heads, all) = self
            .doc()
            .read(|d| (doc::heads_string(d), doc::read_all(d)))
            .await
            .map_err(internal)?;
        let all = all.map_err(internal)?;
        self.model
            .lock()
            .map_err(internal)?
            .rebuild(&all, &heads)
            .map_err(internal)
    }

    async fn read(&self, url: &str) -> Result<Option<Bookmark>> {
        let key = url.to_string();
        self.doc()
            .read(move |d| doc::read(d, &key))
            .await
            .map_err(internal)?
            .map_err(internal)
    }

    /// Applies a change, flushes to disk and refreshes the read model.
    async fn commit<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(
                &mut automerge::transaction::Transaction<'_>,
            ) -> std::result::Result<T, doc::DocError>
            + Send
            + 'static,
    {
        let value = self
            .doc()
            .change(move |tx| f(tx).map_err(change_err))
            .await
            .map_err(|e| match e {
                automerge_repo::Error::Change(m) if m.starts_with("not found") => {
                    Failure(ErrorCode::NotFound, m)
                }
                other => internal(other),
            })?
            .value;
        self.repo()
            .flush()
            .await
            .map_err(|e| Failure(ErrorCode::Io, e.to_string()))?;
        self.reconcile().await?;
        Ok(value)
    }

    pub fn post(
        &self,
        event: &'static str,
        origin: &str,
        bookmark: &Bookmark,
        previous: Option<&Bookmark>,
    ) {
        let payload = hooks::payload(
            event,
            origin,
            &to_json(bookmark),
            previous.map(to_json).as_ref(),
        );
        let _ = self.post.send(PostJob {
            event,
            origin: origin.to_string(),
            url: bookmark.url.clone(),
            payload,
            peer: None,
        });
    }

    pub async fn handle(self: &Arc<Self>, request: Request) -> Response {
        match self.dispatch(request).await {
            Ok(value) => Response::ok(value),
            Err(Failure(code, message)) => Response::err(code, message),
        }
    }

    async fn dispatch(self: &Arc<Self>, request: Request) -> Result<Value> {
        match request {
            Request::Ping => Ok(json!({ "version": wobook_core::VERSION })),
            Request::Add {
                url,
                title,
                description,
                tags,
                fetch,
                merge,
                origin,
            } => {
                let record = Record {
                    url,
                    title,
                    description,
                    tags,
                    fetch: Some(fetch),
                    ..Default::default()
                };
                self.add(record, merge, &origin).await
            }
            Request::Update {
                url,
                title,
                description,
                tags,
                add_tags,
                remove_tags,
                origin,
            } => {
                let url = norm_url(&url)?;
                let _guard = self.writes.lock().await;
                let previous = self.read(&url).await?.ok_or_else(|| not_found(&url))?;
                let key = url.clone();
                let now = now_ms();
                let tags = tags.map(|t| norm_tags(&t));
                let add = add_tags.map(|t| norm_tags(&t));
                let remove = remove_tags.map(|t| norm_tags(&t));
                self.commit(move |tx| {
                    doc::set_fields(tx, &key, title.as_deref(), description.as_deref(), now)?;
                    if let Some(tags) = &tags {
                        doc::set_tags(tx, &key, tags, now)?;
                    }
                    if let Some(remove) = &remove {
                        doc::remove_tags(tx, &key, remove, now)?;
                    }
                    if let Some(add) = &add {
                        doc::add_tags(tx, &key, add, now)?;
                    }
                    Ok(())
                })
                .await?;
                let bookmark = self.read(&url).await?.ok_or_else(|| not_found(&url))?;
                self.post("post-update", &origin, &bookmark, Some(&previous));
                Ok(to_json(&bookmark))
            }
            Request::Rename { from, to, origin } => {
                let from = norm_url(&from)?;
                let to = norm_url(&to)?;
                let _guard = self.writes.lock().await;
                let previous = self.read(&from).await?.ok_or_else(|| not_found(&from))?;
                let now = now_ms();
                let (f, t) = (from.clone(), to.clone());
                let bookmark = self.commit(move |tx| doc::rename(tx, &f, &t, now)).await?;
                self.post("post-update", &origin, &bookmark, Some(&previous));
                Ok(json!({ "from": from, "to": to, "bookmark": to_json(&bookmark) }))
            }
            Request::Delete { url, origin } => self.set_deleted(&url, true, &origin).await,
            Request::Restore { url, origin } => self.set_deleted(&url, false, &origin).await,
            Request::Get { url } => {
                let url = norm_url(&url)?;
                let model = self.model.lock().map_err(internal)?;
                let bookmark = model
                    .get(&url)
                    .map_err(internal)?
                    .ok_or_else(|| not_found(&url))?;
                Ok(to_json(&bookmark))
            }
            Request::List {
                tags,
                include_deleted,
                limit,
            } => {
                let tags = norm_tags(&tags);
                let model = self.model.lock().map_err(internal)?;
                let mut all = model.list(&tags, include_deleted).map_err(internal)?;
                if let Some(limit) = limit {
                    all.truncate(limit);
                }
                Ok(Value::Array(all.iter().map(to_json).collect()))
            }
            Request::Search {
                query,
                tags,
                include_deleted,
                limit,
            } => {
                let tags = norm_tags(&tags);
                let model = self.model.lock().map_err(internal)?;
                let hits = self
                    .searcher
                    .lock()
                    .map_err(internal)?
                    .search(&model, &query, &tags, include_deleted, limit)
                    .map_err(internal)?;
                serde_json::to_value(hits).map_err(internal)
            }
            Request::Tags => {
                let model = self.model.lock().map_err(internal)?;
                let tags = model.tags().map_err(internal)?;
                Ok(Value::Array(
                    tags.into_iter()
                        .map(|(tag, count)| json!({ "tag": tag, "count": count }))
                        .collect(),
                ))
            }
            Request::Import { format, path } => self.import(format, &path).await,
            Request::Export {
                format,
                path,
                include_deleted,
            } => {
                let mut all = {
                    let model = self.model.lock().map_err(internal)?;
                    model.list(&[], include_deleted).map_err(internal)?
                };
                all.sort_by(|a, b| a.created_ms.cmp(&b.created_ms).then(a.url.cmp(&b.url)));
                let content = match format {
                    Format::Jsonl => interchange::write_jsonl(&all),
                    Format::Netscape => interchange::write_netscape(&all),
                    Format::Buku => {
                        return Err(Failure(
                            ErrorCode::InvalidRequest,
                            "cannot export to buku".into(),
                        ));
                    }
                };
                match path {
                    Some(path) => {
                        std::fs::write(&path, content)
                            .map_err(|e| Failure(ErrorCode::Io, format!("{path}: {e}")))?;
                        Ok(json!({ "path": path, "count": all.len() }))
                    }
                    None => Ok(json!({ "content": content, "count": all.len() })),
                }
            }
            Request::Status => {
                let heads = self.doc().read(doc::heads_string).await.map_err(internal)?;
                let (live, deleted) = self
                    .model
                    .lock()
                    .map_err(internal)?
                    .counts()
                    .map_err(internal)?;
                let mut status = json!({
                    "version": wobook_core::VERSION,
                    "data_dir": self.data_dir,
                    "socket": self.socket,
                    "bookmark_count": live,
                    "deleted_count": deleted,
                    "heads": heads,
                    "uptime_s": self.started.elapsed().as_secs(),
                    "hooks_dir": self.hooks_dir,
                    "device": self.sync.device_json(),
                    "peers": self.sync.peer_counts(),
                });
                if let Some(recovery) = self.recovery_status() {
                    status["recovery"] = recovery;
                }
                Ok(status)
            }
            Request::Hooks => Ok(Value::Array(
                hooks::list(&self.hooks_dir)
                    .into_iter()
                    .map(|(event, path)| json!({ "event": event, "path": path }))
                    .collect(),
            )),
            Request::RunHooks { event, url } => {
                if !hooks::EVENTS.contains(&event.as_str()) {
                    return Err(Failure(
                        ErrorCode::InvalidRequest,
                        format!("unknown event: {event}"),
                    ));
                }
                let url = norm_url(&url)?;
                let bookmark = self.read(&url).await?.ok_or_else(|| not_found(&url))?;
                let payload = hooks::payload(&event, "cli", &to_json(&bookmark), None);
                let ctx = hooks::Context {
                    event: &event,
                    origin: "cli",
                    url: &url,
                    data_dir: &self.data_dir,
                    peer: None,
                };
                let outcomes = hooks::run_all(&self.hooks_dir, &ctx, &payload).await;
                serde_json::to_value(outcomes).map_err(internal)
            }
            Request::Shutdown => {
                self.shutdown.notify_one();
                Ok(json!({}))
            }
            other => self.sync_request(other).await,
        }
    }

    async fn set_deleted(&self, url: &str, deleted: bool, origin: &str) -> Result<Value> {
        let url = norm_url(url)?;
        let _guard = self.writes.lock().await;
        let previous = self.read(&url).await?.ok_or_else(|| not_found(&url))?;
        let key = url.clone();
        let now = now_ms();
        self.commit(move |tx| {
            if deleted {
                doc::tombstone(tx, &key, now)
            } else {
                doc::restore(tx, &key, now)
            }
        })
        .await?;
        let bookmark = self.read(&url).await?.ok_or_else(|| not_found(&url))?;
        let event = if deleted {
            "post-delete"
        } else {
            "post-update"
        };
        self.post(event, origin, &bookmark, Some(&previous));
        Ok(to_json(&bookmark))
    }

    async fn pre_add(&self, mut record: Record, origin: &str) -> Result<Record> {
        for hook in hooks::discover(&self.hooks_dir, "pre-add") {
            let current = serde_json::to_value(&record).map_err(internal)?;
            let payload = hooks::payload("pre-add", origin, &current, None);
            let ctx = hooks::Context {
                event: "pre-add",
                origin,
                url: &record.url,
                data_dir: &self.data_dir,
                peer: None,
            };
            let outcome = hooks::run_one(&hook, &ctx, &payload).await;
            hooks::log_outcome("pre-add", &outcome);
            if !outcome.success() {
                let message = outcome.stderr.trim();
                let message = if message.is_empty() {
                    format!("rejected by {}", outcome.hook)
                } else {
                    message.to_string()
                };
                return Err(Failure(ErrorCode::HookRejected, message));
            }
            let out = outcome.stdout.trim();
            if !out.is_empty() {
                match serde_json::from_str::<Record>(out) {
                    Ok(mut replacement) => {
                        if replacement.fetch.is_none() {
                            replacement.fetch = record.fetch;
                        }
                        record = replacement;
                    }
                    Err(e) => eprintln!("wobookd: pre-add {} output ignored: {e}", outcome.hook),
                }
            }
        }
        Ok(record)
    }

    async fn add(&self, record: Record, merge: bool, origin: &str) -> Result<Value> {
        let mut record = record;
        record.url = norm_url(&record.url)?;
        record.tags = norm_tags(&record.tags);
        let mut record = self.pre_add(record, origin).await?;
        record.url = norm_url(&record.url)?;
        record.tags = norm_tags(&record.tags);

        if let Some(existing) = self.read(&record.url).await?
            && !existing.deleted
            && !merge
        {
            return Err(Failure(
                ErrorCode::Exists,
                format!("exists: {}", existing.url),
            ));
        }

        let title_given = record.title.as_deref().is_some_and(|t| !t.is_empty());
        let fetch_status = if title_given || record.fetch == Some(false) {
            "skipped"
        } else {
            match fetch::fetch_metadata(&record.url, FetchLimits::default()).await {
                Ok(meta) => {
                    if record.title.as_deref().unwrap_or("").is_empty() {
                        record.title = meta.title;
                    }
                    if record.description.as_deref().unwrap_or("").is_empty() {
                        record.description = meta.description;
                    }
                    "ok"
                }
                Err(e) => {
                    eprintln!("wobookd: fetch {} failed: {e}", record.url);
                    "failed"
                }
            }
        };

        let _guard = self.writes.lock().await;
        let previous = self.read(&record.url).await?;
        if let Some(existing) = &previous
            && !existing.deleted
            && !merge
        {
            return Err(Failure(
                ErrorCode::Exists,
                format!("exists: {}", existing.url),
            ));
        }
        let now = now_ms();
        let incoming = Bookmark {
            url: record.url.clone(),
            title: record.title.unwrap_or_default(),
            description: record.description.unwrap_or_default(),
            tags: record.tags,
            created_ms: now,
            updated_ms: now,
            deleted: false,
        };
        let (bookmark, kind) = combine(previous.as_ref(), incoming, now);
        if kind != "unchanged" {
            let b = bookmark.clone();
            self.commit(move |tx| doc::upsert(tx, &b)).await?;
            self.post("post-add", origin, &bookmark, previous.as_ref());
        }
        Ok(json!({
            "bookmark": to_json(&bookmark),
            "created": kind == "created",
            "merged": kind == "merged" || kind == "unchanged",
            "restored": kind == "restored",
            "fetch": fetch_status,
        }))
    }

    pub async fn import(&self, format: Option<Format>, path: &str) -> Result<Value> {
        let path_buf = PathBuf::from(path);
        let format = format.or_else(|| Format::infer(&path_buf)).ok_or_else(|| {
            Failure(
                ErrorCode::InvalidRequest,
                format!("cannot infer format of {path}"),
            )
        })?;
        let io = |e: std::io::Error| Failure(ErrorCode::Io, format!("{path}: {e}"));
        let now = now_ms();
        let parsed = match format {
            Format::Jsonl => {
                interchange::read_jsonl(&std::fs::read_to_string(&path_buf).map_err(io)?)
            }
            Format::Netscape => {
                interchange::read_netscape(&std::fs::read_to_string(&path_buf).map_err(io)?)
            }
            Format::Buku => interchange::read_buku(&path_buf, now)
                .map_err(|e| Failure(ErrorCode::Io, format!("{path}: {e}")))?,
        };
        let mut report = ImportReport {
            errors: parsed.errors,
            ..Default::default()
        };
        let mut incoming = Vec::new();
        for (line, record) in parsed.records {
            match norm_url(&record.url) {
                Ok(url) => incoming.push(Bookmark {
                    url,
                    title: record.title.unwrap_or_default(),
                    description: record.description.unwrap_or_default(),
                    tags: norm_tags(&record.tags),
                    created_ms: record.created_ms.unwrap_or(now),
                    updated_ms: record.updated_ms.or(record.created_ms).unwrap_or(now),
                    deleted: false,
                }),
                Err(Failure(_, message)) => {
                    report.errors.push(interchange::LineError { line, message })
                }
            }
        }
        let _guard = self.writes.lock().await;
        let mut report = self
            .commit(move |tx| {
                for b in incoming {
                    let previous = doc::read(&*tx, &b.url)?;
                    let updated_ms = b.updated_ms;
                    let (mut merged, kind) = combine(previous.as_ref(), b, now);
                    match kind {
                        "unchanged" => {
                            report.skipped += 1;
                            continue;
                        }
                        "created" => {
                            merged.updated_ms = updated_ms;
                            report.added += 1;
                        }
                        _ => report.merged += 1,
                    }
                    doc::upsert(tx, &merged)?;
                }
                Ok(report)
            })
            .await?;
        report.errors.sort_by_key(|e| e.line);
        serde_json::to_value(report).map_err(internal)
    }
}

/// Merge rules shared by `add` and `import`. Returns the bookmark to write and
/// one of `created`, `restored`, `merged`, `unchanged`.
fn combine(previous: Option<&Bookmark>, incoming: Bookmark, now: i64) -> (Bookmark, &'static str) {
    match previous {
        None => (incoming, "created"),
        Some(old) if old.deleted => {
            let mut b = old.clone();
            if !incoming.title.is_empty() {
                b.title = incoming.title.clone();
            }
            if !incoming.description.is_empty() {
                b.description = incoming.description.clone();
            }
            doc::merge_into(&mut b, &incoming);
            b.deleted = false;
            b.updated_ms = now;
            (b, "restored")
        }
        Some(old) => {
            let mut b = old.clone();
            doc::merge_into(&mut b, &incoming);
            if b == *old {
                return (b, "unchanged");
            }
            b.updated_ms = now;
            (b, "merged")
        }
    }
}
