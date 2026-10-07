## Context

Repo is empty apart from OpenSpec, prompts and the Android design bundle. The user runs buku through a `buku-fzf` script (`/home/gean/nix-config/modules/shared/programs/cli/buku/scripts/buku-fzf.sh`) launched from a Wayland keybinding (`tui-bookmarks.sh` opens kitty running `buku-fzf --select|--add|--edit`). Current data: 86 bookmarks in `~/.local/share/buku/bookmarks.db`, schema `bookmarks(id, URL UNIQUE, metadata, tags ',a,b,', desc, flags)`. Several tags contain spaces (`ui library`, `ai agent`).

Reference code to copy from:
- `/home/gean/Sandboxes/sandbox/fi/crates/automerge_repo` (whole crate, copied verbatim).
- `/home/gean/Sandboxes/sandbox/session-tap/crates/sessiontap-infra/src/{socket.rs,fs.rs,sqlite.rs,json.rs}` for private socket + lock, atomic write, private SQLite, JSON lines.
- `/home/gean/Sandboxes/sandbox/fi/crates/app_core/src/projection.rs` for the "rebuild read model when heads differ" pattern.
- `/home/gean/Sandboxes/sandbox/vendor/buku/buku.py` for `to_temp_file_content` / `parse_temp_file_content` (editor template), `fetch_data` (title/description extraction), Netscape HTML export/import.

Later changes add `wobook-sync` (QUIC, pairing, mDNS), the Android app over UniFFI, the browser extension and the Nix module. This change must not block them: the document format, storage layout and daemon API are the contract.

## Goals / Non-Goals

**Goals:**
- One Linux machine can replace buku: add, search, edit, delete, import the existing DB, same fzf UX.
- Automerge document is authoritative, SQLite read model is disposable.
- Single writer (`wobookd`), every other process is a client over a unix socket.
- Hooks let the user script around events without touching Rust.
- Everything builds and tests inside `nix develop`.

**Non-Goals:**
- Networking, pairing, sync (p2p-sync).
- Android, UniFFI (android-app).
- Browser extension and native messaging (browser-extension).
- Home-manager module, systemd unit packaging (nix-module); this change only ships a plain unit file in `contrib/`.
- Encryption at rest, browser bookmark database import, integer bookmark ids, ratatui TUI.

## Decisions

### D1. Workspace layout and toolchain

```
Cargo.toml                 workspace, edition 2024, rust-version 1.90, unsafe_code = "forbid", clippy -D warnings
crates/automerge_repo/     verbatim copy of fi's crate (package name automerge-repo, lib automerge_repo)
crates/wobook-core/        domain + projection + search + fetch + import/export, no sockets
crates/wobookd/            daemon binary
crates/wobook/             CLI binary
contrib/wobook-fzf.sh, contrib/hooks/{pre-add.strip-utm,post-add.notify}, contrib/wobookd.service
flake.nix                  dev shell: rust toolchain (1.90+), cargo-nextest optional, sqlite, fzf, pkg-config, openssl not needed (rustls)
```
Pin versions like fi: `automerge = "=0.11.0"`, `rusqlite` with `bundled`, `tokio` full, `serde`, `serde_json`, `clap` derive, `url`, `uuid` v7, `nucleo` (the matcher crate, not nucleo-picker), `reqwest` with `rustls-tls` and no default features, `scraper` for HTML parsing, `thiserror`, `anyhow` only in binaries, `fs2` for locks, `tempfile` in tests, `chrono` or `time` for formatting.

Alternative considered: no daemon, CLI opens the Automerge file directly. Rejected: p2p-sync needs a long-lived listener and two processes must never write the same document directory.

### D2. Bookmark identity = normalized URL

`wobook_core::url::normalize(input) -> Result<NormalizedUrl, UrlError>`:
1. Trim whitespace. Reject empty. If no scheme, prepend `https://`.
2. Parse with the `url` crate. Reject non http(s) unless it parses as a URL with another known scheme (`ftp`, `file`, `magnet` are allowed and kept verbatim after step 3).
3. Lowercase scheme and host. Remove default port (80 for http, 443 for https). Drop the fragment. Keep query as-is. Keep path and trailing slash as-is. Percent-decode unreserved characters only (what `url` does by default).
4. Key is the resulting string.

Not done in core: stripping `utm_*`, `fbclid` etc. That is the shipped `pre-add` example hook.

Editing a URL is `move`: tombstone the old key, create the new key with the same title, description, tags, created_ms. The CLI exposes it as `wobook edit <url>` when the URL line in the editor changes, and as `wobook mv <old> <new>`.

Alternative: uuid v7 keys. Rejected: concurrent "save same URL on two devices" would create duplicates and need a dedupe pass; URL keys merge for free and match buku's UNIQUE URL.

### D3. Tag rules

`wobook_core::tags::parse(input: &str) -> Vec<Tag>`: split on `,`, trim, lowercase, collapse internal whitespace runs to one space, drop empty, dedupe preserving first occurrence, sort for display. A tag never contains a comma. Spaces inside are allowed (`ui library`). Tags are stored as a set keyed by the normalized tag string.

### D4. Automerge document layout

One document, created by `Repo::initialize_new()` on first run; its `DocumentId` is recorded by `automerge_repo`'s bootstrap record so later runs call `repo.get(root_id)`.

```
ROOT
└── "bookmarks": Map<url_key, Map>
      "url":         Str      (the normalized URL, duplicated for convenience)
      "title":       Str
      "description": Str
      "tags":        Map<tag, bool true>     add-wins set (Automerge: concurrent put beats delete)
      "created_ms":  Int
      "updated_ms":  Int                     wall clock of last local write, display only
      "deleted":     Bool                    tombstone, default absent = false
└── "meta": Map { "schema": Int 1 }
```
Scalars use plain `tx.put`; Automerge's deterministic LWW is enough for one human. No HLC (fi's `write_lww_register` is not copied). Readers use `doc.get` (winner) and ignore `get_all` conflicts. Undelete sets `deleted=false`. A `wobook compact` is a non-goal; the document stays append-only.

`wobook_core::doc` exposes pure functions over `&Automerge` / `&mut Transaction`:
`upsert(tx, Bookmark)`, `set_tags`, `add_tags`, `remove_tags`, `tombstone(tx, key)`, `restore(tx, key)`, `rename(tx, old, new)`, `read_all(doc) -> Vec<Bookmark>`, `read(doc, key) -> Option<Bookmark>`. These functions are the only place that knows the layout; Android (change 3) reuses them through UniFFI.

### D5. Storage layout

```
$XDG_DATA_HOME/wobook/           (default; override with WOBOOK_DATA_DIR or --data-dir)
├── automerge/<uuid>.automerge   FilesystemStorage snapshots (owned by automerge_repo)
├── control/bootstrap-v1.bin     automerge_repo bootstrap record
├── quarantine/                  automerge_repo
├── read-model.sqlite            disposable, deletable at any time
├── daemon.lock                  fs2 exclusive lock, held for the daemon lifetime
└── hooks/                       NOT here; hooks live in $XDG_CONFIG_HOME/wobook/hooks (see D9)
$XDG_RUNTIME_DIR/wobook/wobookd.sock   (fallback $XDG_DATA_HOME/wobook/wobookd.sock)
```
`wobookd` is the only process that opens `automerge/` and `read-model.sqlite`. `automerge_repo::FilesystemStorage` is used for both `StorageAdapter` and `ControlStore`. A `NullTransport` implementing `NetworkTransport` (never emits events, `send` returns an error, `close` ok) is passed to `Repo::open`.

### D6. Read model (SQLite) and projection

```sql
CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT);          -- schema_version, heads_checkpoint
CREATE TABLE bookmarks(url TEXT PRIMARY KEY, title TEXT, description TEXT, created_ms INTEGER, updated_ms INTEGER, deleted INTEGER);
CREATE TABLE tags(url TEXT REFERENCES bookmarks(url) ON DELETE CASCADE, tag TEXT, PRIMARY KEY(url, tag));
CREATE INDEX tags_tag ON tags(tag);
```
`Projection::reconcile(&Automerge)`: compute heads string (sorted change hashes joined by `,`, prefixed `v1:`); if equal to `meta.heads_checkpoint` return; else inside one transaction delete everything, insert `read_all(doc)`, write checkpoint. Full rebuild on every change, same as fi; fine at bookmark scale (thousands). If the SQLite file fails to open or `schema_version` mismatches, delete it and rebuild. The daemon calls `reconcile` after every local change and on every `DocumentEvent` from the repo.

Queries (`list`, `search`, `tags`) read the SQLite table and then run nucleo in memory over the candidate rows.

### D7. Fuzzy search

`wobook_core::search::Searcher` wraps `nucleo::Matcher` with `nucleo::pattern::Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart)`. Haystack per bookmark is `title + "\n" + url + "\n" + description + "\n" + tags.join(" ")`; score = nucleo score; ties broken by `updated_ms` desc. Returns `Vec<Hit { bookmark, score, indices: Vec<u32> }>` where indices are char positions in the haystack so clients (Android later) can highlight. Optional `tags: &[Tag]` filter is applied in SQL first (`url IN (SELECT url FROM tags WHERE tag = ?...)` AND semantics). Empty query returns all, ordered by `created_ms` desc. Deleted rows are never returned unless `include_deleted`.

### D8. Metadata fetch

`wobook_core::fetch::fetch_metadata(url, FetchLimits) -> Result<Metadata { title: Option<String>, description: Option<String> }, FetchError>`. reqwest client with rustls, 8 s total timeout, follow up to 5 redirects, read at most 512 KiB of the body, only when `Content-Type` is `text/html` or `application/xhtml+xml`, decode with the charset from the header or `<meta charset>`. Extract `<title>`, then `og:title` as fallback; description from `<meta name="description">`, then `og:description`. Collapse whitespace, cap title at 512 chars and description at 4096. Any error returns `FetchError` and the caller continues with empty fields. Daemon runs the fetch on `add` unless the request has `fetch: false` or the title was supplied. `User-Agent: wobook/<version>`.

### D9. Daemon API (unix socket, JSON lines)

Bind with the session-tap recipe: create private dir (0700), acquire `daemon.lock` with `fs2::FileExt::try_lock_exclusive` (fail fast with "wobookd already running"), unlink stale socket, bind, chmod 0600. One connection = one request = one response (newline-delimited JSON, UTF-8, max 4 MiB request), connection closes after the response. Serde enums tagged `type`:

Requests
```
{"type":"ping"}
{"type":"add","url":"...","title":null,"description":null,"tags":["a","b"],"fetch":true,"origin":"cli"}
{"type":"update","url":"...","title":?,"description":?,"tags":?,"add_tags":?,"remove_tags":?}
{"type":"rename","from":"...","to":"..."}
{"type":"delete","url":"..."}       {"type":"restore","url":"..."}
{"type":"get","url":"..."}
{"type":"list","tags":[],"include_deleted":false,"limit":null}
{"type":"search","query":"shcn ui","tags":[],"limit":50}
{"type":"tags"}                                   -> [{tag, count}]
{"type":"import","format":"jsonl|netscape|buku","path":"/abs/path"}
{"type":"export","format":"jsonl|netscape","path":"/abs/path or null for inline"}
{"type":"status"}                                 -> version, data_dir, bookmark_count, heads, uptime, hooks_dir
{"type":"shutdown"}
```
Responses: `{"ok":true,"result":...}` or `{"ok":false,"error":{"code":"invalid_url|not_found|exists|hook_rejected|io|internal","message":"..."}}`. Bookmark JSON shape is the JSONL record (D10). `add` on an existing non-deleted URL returns `exists` unless `merge:true`, in which case tags are unioned and empty fields filled (this is what the fzf wrapper and import use). `add` on a tombstoned URL restores it and applies the new fields.

Protocol types live in `wobook_core::protocol` so `wobook`, `wobookd` and later the browser native host share them.

### D10. JSONL record and import/export

```json
{"url":"https://excalidraw.com/","title":"Excalidraw","description":"...","tags":["drawing","tool"],"created_ms":1759800000000,"updated_ms":1759800000000}
```
Deleted bookmarks are omitted from export unless `--include-deleted`, which adds `"deleted":true`. Import is `add` with `merge:true` per record, never fails the whole file on one bad line; the response reports `{added, merged, skipped, errors:[{line, message}]}`.

Netscape HTML: `<DT><A HREF ADD_DATE TAGS>title</A>` + optional `<DD>description`; `ADD_DATE` is seconds. Import tolerates nested `<DL>` folders and ignores folder names. buku import reads `bookmarks(URL, metadata, tags, desc)` with rusqlite in read-only mode (`?mode=ro`), tags split on `,` and run through D3, `metadata` becomes title, `flags` ignored, `created_ms` = import time (buku stores none).

### D11. Hooks

Directory `$XDG_CONFIG_HOME/wobook/hooks/` (override `WOBOOK_HOOKS_DIR`). Any executable file named `<event>` or `<event>.<anything>` runs, sorted by name. Events: `pre-add`, `post-add`, `post-update`, `post-delete`, `post-sync` (emitted by p2p-sync later, defined now). Contract:
- stdin: one JSON object `{"event":"post-add","origin":"cli|extension|android|remote:<device>","bookmark":{...record...},"previous":{...}|null}`.
- env: `WOBOOK_EVENT`, `WOBOOK_ORIGIN`, `WOBOOK_URL`, `WOBOOK_DATA_DIR`.
- `pre-add` only: stdout may contain a full replacement record JSON; exit 0 = accept (with replacement if stdout non-empty and parses), exit 1 = reject with `hook_rejected` and stderr as message. Runs before the fetch so a hook can supply a title and skip fetching by setting `"fetch":false` in the replacement.
- `post-*`: fire-and-forget after the write is durable (`repo.flush()`), stdout/stderr logged, 10 s timeout then SIGKILL, failures never affect the write. Run sequentially per event in a dedicated tokio task so they never block the socket.

Shipped examples (`contrib/hooks/`, executable, bash + jq): `pre-add.strip-utm` removes `utm_*`, `fbclid`, `gclid` query params; `post-add.notify` runs `notify-send` only when origin starts with `remote:`.

### D12. CLI

`wobook` uses clap derive. Connects to the socket; if the daemon is not running, prints `wobookd is not running (start it: wobookd, or systemctl --user start wobookd)` and exits 69 (EX_UNAVAILABLE). No direct-file fallback in this change.

```
wobook add <url> [-t tag,tag] [--title T] [--desc D] [--no-fetch] [--merge]
wobook edit <url>            opens $EDITOR with the buku-style template (URL, TITLE, TAGS comma-separated, DESC multi-line), diff applied as update or rename
wobook mv <old> <new>
wobook rm <url>... [--restore]
wobook show <url> [--json]
wobook list [-t tag,tag] [--format tsv|json|jsonl|pretty] [--include-deleted]
wobook search <query...> [-t tag] [--format ...] [--limit N]
wobook tags [--format tsv|json]
wobook import <path> [--format jsonl|netscape|buku]   (format inferred from extension .jsonl/.html/.db)
wobook export [path] [--format jsonl|netscape] [--include-deleted]
wobook status [--json]
wobook hooks list|run <event> <url>   (run = replay for testing)
wobook completions <shell>
```
`--format tsv` for `list`/`search` prints `url\ttitle\ttags(comma)` with tabs/newlines in fields replaced by spaces; this is the fzf input (URL in column 1 replaces buku's id). `pretty` prints buku-like blocks. Exit codes: 0 ok, 1 not found / invalid input, 2 usage, 69 daemon unavailable, 70 internal.

### D13. wobook-fzf.sh

Copy `buku-fzf.sh` behaviour exactly, swapping commands:
- `--select`: `wobook list --format tsv | fzf --with-nth=2.. --delimiter='\t' --preview 'wobook show {1}' --reverse --preview-window=wrap | cut -f1 | wl-copy --trim-newline`.
- `--open`: same selection, `xdg-open "$url"`.
- `--add`: `wobook edit --new` (template with empty URL, same as `buku --write`).
- `--edit`: loop like the original: select, `wobook edit "$url"`, ask "Do you want to continue editing? (y/n)".
- No args: print `Available Options : --select --open --add --edit`.
Also `contrib/wobookd.service` (systemd user unit, `ExecStart=wobookd`, `Restart=on-failure`) for manual installation until the nix-module change.

### D14. Testing strategy

e2e first: `crates/wobookd/tests/e2e.rs` spawns the built `wobookd` binary with a temp data dir and socket, drives it through the built `wobook` binary (`assert_cmd`), covers add/fetch-disabled/edit/rename/rm/restore/list/search/tags/import buku fixture/export roundtrip/hooks reject and rewrite/duplicate add/daemon-down exit code/read-model deletion and rebuild. Unit tests only for `url::normalize`, `tags::parse`, `doc` merge semantics (two `Automerge` instances, concurrent tag add + remove, concurrent same-URL add, rename vs edit) and the editor template parser. Fetch tests use a local `tokio` HTTP server fixture, never the network. A buku fixture DB is generated in the test from the known schema.

## Risks / Trade-offs

- [Full read-model rebuild on each change] → bookmark scale is thousands at most; measured in e2e with 5k records to stay under 200 ms. Incremental projection is a later optimization.
- [automerge_repo bootstrap semantics (initialize_new vs join_existing) are heavier than needed for one document] → wrap in `wobook_core::store::open_or_init(data_dir)` so the rest of the code sees one `DocHandle`; join path is only used by p2p-sync.
- [URL normalization merges pages the user considers distinct (fragment-only SPAs)] → fragments are dropped deliberately; documented; `#!`-style hashbang URLs are kept because `url` treats them as fragments too, so add an exception: keep the fragment when it starts with `!`.
- [Hooks can hang or spam] → timeouts, sequential runner, never on the request path except `pre-add`, which has a 10 s timeout and counts as reject on timeout.
- [Daemon not running when the keybinding fires] → clear exit 69 message; nix-module change will make it a user service with socket activation considered.
- [Editing the URL in `$EDITOR` is a rename, surprising] → the template says so in a comment line, and `wobook edit` prints "moved <old> -> <new>".

## Migration Plan

1. `nix develop`, `cargo build`, start `wobookd` manually.
2. `wobook import ~/Sync/.config/buku/bookmarks.db` (tags with spaces preserved).
3. Swap the kitty keybinding script target from `buku-fzf` to `wobook-fzf` on one host; keep buku installed until p2p-sync lands on a second host.
4. Rollback: buku DB is untouched; `wobook export` produces JSONL anytime.

## Open Questions

- None blocking. `wobook edit --new` template keeps buku's "Leave blank to web fetch, `-` for no title" convention.
