## Why

buku stores bookmarks in one SQLite file synced by Syncthing. Concurrent writers corrupt the file and the Android phone can only read. wobook replaces it with a local-first core where an Automerge document is the source of truth and SQLite is a disposable read model. This change delivers the single-device foundation: Rust core, daemon, CLI, hooks, import/export and the fzf wrapper, so one Linux machine can drop buku today. Sync, Android, browser extension and Nix packaging follow in later changes.

## What Changes

- New Rust workspace at `crates/` with `automerge_repo` (copied verbatim from fi), `wobook-core`, `wobookd` and `wobook`.
- `wobook-core`: bookmark domain (URL normalization as identity, tag rules), Automerge document schema with field-level LWW and add-wins tag set, SQLite read model projected from Automerge heads, nucleo fuzzy search, title/description fetch, JSONL and Netscape HTML import/export, buku SQLite import.
- `wobookd`: single-writer daemon owning the Automerge file and read model, serving a JSON-lines API over a private unix socket, running git-style hooks on bookmark events.
- `wobook` CLI: add, edit ($EDITOR template), rm, show, list, search, tags, import, export, status, hooks. Thin client over the daemon socket.
- `contrib/wobook-fzf.sh`: byte-compatible with the user's current `buku-fzf` (`--select` copies with wl-copy, `--add`, `--edit` loop) plus `--open` via xdg-open.
- `contrib/hooks/`: example hooks (strip `utm_*` params in `pre-add`, `notify-send` on remote adds).
- Nix flake with a dev shell (Rust toolchain, sqlite, fzf) so the workspace builds and tests under `nix develop`.

## Capabilities

### New Capabilities
- `bookmark-model`: bookmark fields, URL normalization as identity, tag normalization, tombstone deletes, Automerge document layout and merge semantics.
- `read-model-projection`: disposable SQLite read model rebuilt from Automerge heads, checkpointing, recovery by deletion.
- `fuzzy-search`: nucleo-based fuzzy matching over title, URL, description and tags with match positions, tag filtering, ordering.
- `metadata-fetch`: fetch title and description for a URL with timeouts, size limits, failure tolerance, and an opt-out.
- `daemon-api`: wobookd lifecycle, single-writer storage ownership, private unix socket JSON-lines protocol, commands and errors.
- `cli`: wobook command surface, output formats, editor flow, exit codes, behaviour when the daemon is down.
- `hooks`: hook directory, events, payload contract, pre-add rewrite and reject semantics, failure isolation.
- `import-export`: JSONL canonical format, Netscape bookmarks HTML, direct buku SQLite import, upsert-by-URL merge rules.
- `fzf-wrapper`: wobook-fzf script behaviour compatible with the existing buku-fzf script.

### Modified Capabilities
<!-- none: first change in the repo -->

## Impact

- New code only; no existing specs. Repo currently has no source.
- External crates: automerge 0.11, rusqlite (bundled), nucleo, reqwest (rustls), scraper or similar HTML title extraction, tokio, serde, clap, uuid v7, url.
- Reuses `sessiontap-infra` patterns (private unix socket with lock file, atomic file write, private SQLite open) by copying the relevant functions, not by depending on session-tap.
- Prepares for `p2p-sync`: the daemon already drives `automerge_repo` with a no-op `NetworkTransport`, so `wobook-sync` only has to supply a real transport later; the on-disk document format does not change.
- User's NixOS `buku` module stays untouched; a wobook Nix module ships in a later change.
