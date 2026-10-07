## 1. Workspace and toolchain

- [ ] 1.1 Create the Cargo workspace (`Cargo.toml` with edition 2024, rust-version 1.90, `unsafe_code = "forbid"`, shared dependency versions) and `.gitignore` for `target/`, `result`, `.direnv`.
- [ ] 1.2 Copy `/home/gean/Sandboxes/sandbox/fi/crates/automerge_repo` verbatim into `crates/automerge_repo`, add it to the workspace, confirm `cargo test -p automerge-repo` passes.
- [ ] 1.3 Add `flake.nix` with a dev shell providing the Rust toolchain (1.90+), `sqlite`, `fzf`, `shellcheck`, `jq`; `nix develop -c cargo build --workspace` succeeds. Model it on `/home/gean/Sandboxes/sandbox/session-tap/flake.nix` minus the Android shells.
- [ ] 1.4 Scaffold `crates/wobook-core` (lib), `crates/wobookd` (bin) and `crates/wobook` (bin) with empty modules and a shared `wobook_core::VERSION`.

## 2. wobook-core: domain

- [ ] 2.1 Implement `url::normalize` per design D2 (scheme default, lowercase scheme/host, default port removal, fragment drop except `#!`, invalid URL error) with unit tests for every scenario in `bookmark-model`.
- [ ] 2.2 Implement `tags::parse` and `Tag` newtype per D3 with unit tests (spaces inside tags, dedupe, lowercase, commas never inside).
- [ ] 2.3 Define `Bookmark` and the JSONL `Record` serde type (`url, title, description, tags, created_ms, updated_ms, deleted`) in `wobook_core::model`.
- [ ] 2.4 Implement `doc` functions over Automerge (`upsert`, `set_tags`, `add_tags`, `remove_tags`, `tombstone`, `restore`, `rename`, `read`, `read_all`, `ensure_schema`) per D4 using plain `put` and a tag map for add-wins.
- [ ] 2.5 Unit-test merge semantics with two `Automerge` instances: concurrent tag add vs remove (add wins), concurrent same-URL add (one record, tags unioned), concurrent title edits converge, delete vs concurrent edit, rename onto existing URL.

## 3. wobook-core: storage, projection, search, fetch

- [ ] 3.1 Implement `store::open_or_init(data_dir)` wrapping `automerge_repo::Repo` with `FilesystemStorage` for documents and control, a `NullTransport` implementing `NetworkTransport`, `initialize_new` on fresh storage and `get(root)` afterwards; return a `DocHandle` plus the `Repo`.
- [ ] 3.2 Implement `projection::ReadModel` (schema in D6, `open_or_recreate`, `reconcile(&Automerge)` with heads checkpoint, full rebuild in one transaction, schema-version mismatch recreates the file).
- [ ] 3.3 Implement `search::Searcher` over nucleo per D7 (haystack composition, score + updated_ms ordering, indices and segment boundaries, tag AND filter via SQL, empty query newest first, include_deleted flag).
- [ ] 3.4 Implement `fetch::fetch_metadata` per D8 with reqwest rustls, limits, charset handling, `<title>`/og fallbacks, sanitization; test against an in-process HTTP fixture (HTML, non-HTML, timeout, oversized body).
- [ ] 3.5 Implement `interchange`: JSONL read/write, Netscape HTML read/write (nested folders tolerated), buku SQLite reader (read-only), and the `ImportReport { added, merged, skipped, errors }` type.

## 4. Protocol and daemon

- [ ] 4.1 Define `wobook_core::protocol` request/response enums and error codes exactly as in design D9, with serde `tag = "type"` and a JSON-lines codec (4 MiB cap).
- [ ] 4.2 Implement `wobookd` startup: resolve data dir and socket path (XDG, `WOBOOK_DATA_DIR`, `WOBOOK_SOCKET`, flags), create 0700 dirs, take `daemon.lock` with fs2 (exit non-zero if held), unlink stale socket, bind 0600, open store and read model, initial `reconcile`, log to stderr.
- [ ] 4.3 Implement the request dispatcher: one request per connection, normalization at the boundary, every mutating command followed by `repo.flush()` then `reconcile` before replying; `add` exists/merge/restore semantics; `update` with add_tags/remove_tags; `rename`; `delete`/`restore`; `get`/`list`/`search`/`tags`/`status`/`shutdown`.
- [ ] 4.4 Implement `import`/`export` commands on top of `interchange` (format inference from extension, per-line error collection, idempotent re-import).
- [ ] 4.5 Implement the hooks runner per D11: discovery in `$XDG_CONFIG_HOME/wobook/hooks` or `WOBOOK_HOOKS_DIR`, payload JSON on stdin plus env vars, `pre-add` rewrite/reject/timeout semantics before fetch, `post-*` sequential background task with 10 s timeout, logging only.
- [ ] 4.6 Wire metadata fetch into `add` (skipped when title supplied or `fetch:false` or a `pre-add` rewrite says so) and report `fetch: "ok"|"failed"|"skipped"` in the response.

## 5. CLI

- [ ] 5.1 Implement the `wobook` clap command tree from D12 with a socket client, exit codes (0/1/2/69/70) and the `wobookd is not running` message.
- [ ] 5.2 Implement `list`/`search`/`show`/`tags` output formats (`tsv`, `json`, `jsonl`, `pretty`; TTY default `pretty`, otherwise `tsv`), TSV field sanitization.
- [ ] 5.3 Implement `edit` and `edit --new` with the buku-style template (`$EDITOR`/`$VISUAL`/`vi`), parser for URL/TITLE/TAGS/DESCRIPTION, unchanged file is a no-op, URL change performs rename and prints `moved <old> -> <new>`, blank title triggers fetch, `-` means no title, editor non-zero exit aborts.
- [ ] 5.4 Implement `add`, `mv`, `rm [--restore]`, `import`, `export`, `status`, `hooks list|run`, `completions`.

## 6. contrib

- [ ] 6.1 Write `contrib/wobook-fzf.sh` per D13 (`--select` copies with wl-copy, `--open` via xdg-open, `--add` runs `wobook edit --new`, `--edit` loop with the y/n prompt, usage line otherwise); `shellcheck` clean.
- [ ] 6.2 Write `contrib/hooks/pre-add.strip-utm` (bash + jq, drops `utm_*`, `fbclid`, `gclid` query params and prints the rewritten record) and `contrib/hooks/post-add.notify` (`notify-send` only when origin starts with `remote:`); both executable.
- [ ] 6.3 Write `contrib/wobookd.service` (systemd user unit, `ExecStart=%h/.nix-profile/bin/wobookd` placeholder documented, `Restart=on-failure`) and a short `README.md` at repo root describing install, data dir, socket, hooks and the fzf script.

## 7. End-to-end tests

- [ ] 7.1 Add `crates/wobookd/tests/e2e.rs` helpers: build-and-spawn `wobookd` with temp data dir, socket and hooks dir; run `wobook` via `assert_cmd` against it; teardown with `shutdown`.
- [ ] 7.2 e2e: add (no fetch), show, list tsv shape, search `shcn ui` ranking, tags with counts, tag AND filter, update via `edit` with a scripted editor, rename via edit and `mv`, rm and restore, duplicate add exists vs `--merge`.
- [ ] 7.3 e2e: hooks — `pre-add` rewrite (strip-utm fixture), `pre-add` reject with message, failing `post-add` does not affect result, env vars and payload content, `hooks run` replay.
- [ ] 7.4 e2e: import a generated buku fixture DB (tags with spaces), JSONL export/import roundtrip into a second daemon, Netscape export/import roundtrip, bad JSONL line reported, re-import idempotent, format inference.
- [ ] 7.5 e2e: durability and recovery — SIGKILL after add then restart lists it; delete `read-model.sqlite` and restart rebuilds; second daemon on same dir refused; stale socket replaced; daemon-down exit code 69; metadata fetch against the in-process HTTP fixture (ok, non-HTML, unreachable).
- [ ] 7.6 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and `shellcheck contrib/*.sh` inside `nix develop`; fix everything.
