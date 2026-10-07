# import-export Specification

## Purpose
JSONL canonical format, Netscape bookmarks HTML, direct buku SQLite import, upsert-by-URL merge rules.

## Requirements

### Requirement: JSONL is the canonical interchange format
Export SHALL write one JSON object per line with `url`, `title`, `description`, `tags` (sorted array), `created_ms`, `updated_ms`, omitting tombstoned bookmarks unless `include_deleted` is set, in which case they carry `"deleted": true`. Import SHALL accept the same shape and treat unknown fields as ignored.

#### Scenario: Roundtrip
- **WHEN** a library is exported to JSONL, a fresh daemon imports that file
- **THEN** `list` on the fresh daemon equals the original library (URL, title, description, tags, created_ms)

#### Scenario: Bad line does not abort
- **WHEN** a JSONL file has an unparsable third line
- **THEN** the other records are imported and the result lists one error with `line: 3`

### Requirement: Import merges by URL
Import SHALL upsert each record by normalized URL: new URLs are added, existing URLs get tags unioned and empty title or description filled, and tombstoned URLs are restored. The result SHALL report `added`, `merged`, `skipped` and `errors`.

#### Scenario: Re-import is idempotent
- **WHEN** the same JSONL file is imported twice
- **THEN** the second import reports zero `added`, and the library is unchanged

### Requirement: Netscape bookmarks HTML
Export SHALL produce a Netscape bookmarks file (`<!DOCTYPE NETSCAPE-Bookmark-file-1>`, `<DT><A HREF ADD_DATE TAGS>` with `<DD>` description). Import SHALL read that format, including nested `<DL>` folders, ignoring folder names, mapping `TAGS` to tags and `ADD_DATE` seconds to `created_ms`.

#### Scenario: Browser export imported
- **WHEN** a Firefox-exported bookmarks HTML with nested folders is imported
- **THEN** every `<A HREF>` becomes a bookmark with its title and any `TAGS` attribute as tags

#### Scenario: HTML roundtrip
- **WHEN** a library is exported to Netscape HTML and imported into a fresh daemon
- **THEN** URLs, titles, descriptions and tags match

### Requirement: buku database import
Import with format `buku` SHALL open the SQLite file read-only, read `bookmarks(URL, metadata, tags, desc)`, map `metadata` to title, parse `tags` by splitting on commas with tag normalization, ignore `flags` and set `created_ms` to the import time.

#### Scenario: Import the user's buku database
- **WHEN** a buku DB containing `,ui library,vue,` as tags for a row is imported
- **THEN** the bookmark has tags `{"ui library", "vue"}` and its title equals the buku `metadata` column

#### Scenario: Format inferred from extension
- **WHEN** `wobook import bookmarks.db` runs without `--format`
- **THEN** the buku importer is used; `.jsonl` selects JSONL and `.html` selects Netscape
