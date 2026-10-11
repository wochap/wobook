# fzf-wrapper Specification

## Purpose
Wobook-fzf script behaviour compatible with the existing buku-fzf script.

## Requirements

### Requirement: Compatible with the existing buku-fzf script
`contrib/wobook-fzf.sh` SHALL accept `--select`, `--open`, `--add`, `--edit`. With no argument it SHALL open the picker with the same Enter behaviour as `--open`. It SHALL feed `wobook list --format tsv` to `fzf` with the URL hidden as the key column, a coloured `wobook --color=always show {1}` preview, `--reverse` and `--preview-window=wrap`.

#### Scenario: Select copies URL
- **WHEN** `wobook-fzf --select` runs and the user picks a line
- **THEN** the selected URL is written to the clipboard with `wl-copy --trim-newline` and nothing else is printed

#### Scenario: Open launches browser
- **WHEN** `wobook-fzf --open` runs, or `wobook-fzf` runs with no argument, and the user picks a line
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

### Requirement: Delete bookmark from the picker
In every picker mode, `Ctrl-X` SHALL delete the highlighted bookmark by running `wobook rm <url>`, then reload the list without closing fzf.

#### Scenario: Delete highlighted bookmark
- **WHEN** the user presses `Ctrl-X` on a highlighted bookmark
- **THEN** `wobook rm <url>` is executed for it, the list reloads without it, and fzf stays open

#### Scenario: Cursor stays near deleted row
- **WHEN** a bookmark is deleted from the picker
- **THEN** the cursor lands on the row that took its position (or the last row if it was last)

#### Scenario: Deleting the last remaining bookmark
- **WHEN** the user deletes the only bookmark
- **THEN** fzf stays open with an empty list and Esc exits 0 without side effects

#### Scenario: Deletion is recoverable
- **WHEN** a bookmark was deleted from the picker
- **THEN** it is only tombstoned and `wobook rm --restore <url>` brings it back

### Requirement: Edit bookmark from the picker
In every picker mode, `Ctrl-E` SHALL run `wobook edit <url>` for the highlighted bookmark in the terminal, then reload the list and keep fzf open.

#### Scenario: Edit highlighted bookmark
- **WHEN** the user presses `Ctrl-E` on a highlighted bookmark and saves the editor
- **THEN** the list reloads showing the updated bookmark and fzf stays open

#### Scenario: Edit aborted
- **WHEN** the user quits the editor without saving
- **THEN** the picker resumes with the list unchanged

### Requirement: Copy URL from the picker
In every picker mode, `Ctrl-Y` SHALL copy the highlighted bookmark's URL with `wl-copy --trim-newline` and close the picker without opening or editing it.

#### Scenario: Copy highlighted URL
- **WHEN** the user presses `Ctrl-Y` on a highlighted bookmark
- **THEN** its URL is on the clipboard, nothing is opened, and the script exits 0

### Requirement: Key hints in the picker
The picker SHALL show a header naming the `Enter`, `Ctrl-E`, `Ctrl-Y` and `Ctrl-X` actions for the current mode.

#### Scenario: Header visible
- **WHEN** the picker opens
- **THEN** a header lists the available keys
