# wobook

Local-first bookmarks. An Automerge document is the source of truth; a SQLite
read model is rebuilt from it and can be deleted at any time. `wobookd` is the
only process that touches the data; `wobook` is a thin CLI over its socket.

## Install

```sh
nix develop -c cargo build --release
install -m755 target/release/wobookd target/release/wobook ~/.local/bin/
```

Run the daemon by hand (`wobookd`) or as a user service with
`contrib/wobookd.service` (edit `ExecStart` to where `wobookd` lives).

## Locations

| What | Default | Override |
| --- | --- | --- |
| Data (Automerge, read model, lock) | `$XDG_DATA_HOME/wobook` | `WOBOOK_DATA_DIR`, `wobookd --data-dir` |
| Socket | `$XDG_RUNTIME_DIR/wobook/wobookd.sock` | `WOBOOK_SOCKET`, `--socket` |
| Hooks | `$XDG_CONFIG_HOME/wobook/hooks` | `WOBOOK_HOOKS_DIR`, `wobookd --hooks-dir` |

## Usage

```sh
wobook import ~/.local/share/buku/bookmarks.db   # buku DB, .jsonl or Netscape .html
wobook add example.com/x -t "ui library,react"
wobook search shcn ui
wobook edit https://example.com/x                # $EDITOR; changing the URL moves it
wobook rm https://example.com/x                  # --restore to undo
wobook export backup.jsonl
```

The bookmark identity is the normalized URL (lowercase scheme and host, no
default port, no fragment except `#!`). Tags are comma-separated and may contain
spaces.

## Hooks

Executable files named `<event>` or `<event>.<suffix>` in the hooks directory
run in lexical order for `pre-add`, `post-add`, `post-update`, `post-delete` and
`post-sync`. They get `{"event","origin","bookmark","previous"}` on stdin and
`WOBOOK_EVENT`, `WOBOOK_ORIGIN`, `WOBOOK_URL`, `WOBOOK_DATA_DIR` in the
environment. A `pre-add` hook may print a replacement record (optionally with
`"fetch": false`) or exit non-zero to reject the add (stderr is the message).
`post-*` hooks run in the background with a 10 s timeout and never affect the
result. Examples live in `contrib/hooks/`; try one with
`wobook hooks run post-add <url>`.

## fzf

`contrib/wobook-fzf.sh` replaces `buku-fzf`: `--select` copies a URL with
`wl-copy`, `--open` runs `xdg-open`, `--add` opens an empty editor template,
`--edit` loops over edits.
