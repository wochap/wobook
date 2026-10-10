# fzf-wrapper Specification

## Purpose
Wobook-fzf script behaviour compatible with the existing buku-fzf script.

## Requirements

### Requirement: Compatible with the existing buku-fzf script
`contrib/wobook-fzf.sh` SHALL accept `--select`, `--open`, `--add`, `--edit` and SHALL print `Available Options : --select --open --add --edit` otherwise. It SHALL feed `wobook list --format tsv` to `fzf` with the URL hidden as the key column, a coloured `wobook --color=always show {1}` preview, `--reverse` and `--preview-window=wrap`.

#### Scenario: Select copies URL
- **WHEN** `wobook-fzf --select` runs and the user picks a line
- **THEN** the selected URL is written to the clipboard with `wl-copy --trim-newline` and nothing else is printed

#### Scenario: Open launches browser
- **WHEN** `wobook-fzf --open` runs and the user picks a line
- **THEN** `xdg-open <url>` is executed

#### Scenario: Add opens editor template
- **WHEN** `wobook-fzf --add` runs
- **THEN** `wobook edit --new` is executed

#### Scenario: Edit loop
- **WHEN** `wobook-fzf --edit` runs and the user finishes one edit
- **THEN** the script asks `Do you want to continue editing? (y/n)` and loops on `y`, exits otherwise

#### Scenario: Nothing selected
- **WHEN** the user cancels fzf
- **THEN** the script exits 0 without touching clipboard or browser

#### Scenario: Coloured preview
- **WHEN** the fzf preview pane renders a bookmark
- **THEN** it shows the title, URL and tags in buku's colours

### Requirement: Shell-checkable
The script SHALL pass `shellcheck` with no warnings and use only `bash`, `fzf`, `cut`, `wl-copy` and `xdg-open`.

#### Scenario: Lint
- **WHEN** `shellcheck contrib/wobook-fzf.sh` runs
- **THEN** it exits 0
