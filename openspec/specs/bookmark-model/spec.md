# bookmark-model Specification

## Purpose
Bookmark fields, URL normalization as identity, tag normalization, tombstone deletes, Automerge document layout and merge semantics.

## Requirements

### Requirement: Bookmark identity is the normalized URL
The system SHALL identify a bookmark by its normalized URL. Normalization SHALL trim whitespace, prepend `https://` when no scheme is present, lowercase scheme and host, remove default ports (80 for http, 443 for https), drop the fragment unless it starts with `!`, and keep path, trailing slash and query unchanged.

#### Scenario: Equivalent inputs collapse to one key
- **WHEN** `HTTPS://Example.com:443/docs#intro` and `example.com/docs` are normalized
- **THEN** both produce `https://example.com/docs`

#### Scenario: Query and trailing slash are preserved
- **WHEN** `https://example.com/a/?q=1&utm_source=x` is normalized
- **THEN** the result is `https://example.com/a/?q=1&utm_source=x`

#### Scenario: Hashbang fragment is kept
- **WHEN** `https://example.com/app#!/route` is normalized
- **THEN** the fragment `#!/route` is retained

#### Scenario: Invalid URL rejected
- **WHEN** an empty string or `not a url at all` is normalized
- **THEN** normalization fails with an `invalid_url` error and nothing is stored

### Requirement: Tag normalization
The system SHALL parse tags from comma-separated input by trimming, lowercasing, collapsing internal whitespace to single spaces, dropping empty entries and removing duplicates. A tag SHALL never contain a comma. Spaces inside a tag SHALL be allowed.

#### Scenario: Multi-word tags survive
- **WHEN** tags input is `UI Library, react ,  ai   agent,,react`
- **THEN** the stored tag set is `{"ui library", "react", "ai agent"}`

### Requirement: Bookmark fields
A bookmark SHALL consist of `url` (identity), `title`, `description`, `tags` (set), `created_ms`, `updated_ms` and a `deleted` tombstone flag. Title and description MAY be empty.

#### Scenario: Minimal bookmark
- **WHEN** a bookmark is created with only a URL
- **THEN** it is stored with empty title, empty description, empty tags, `created_ms` and `updated_ms` set to the current wall clock and `deleted` false

### Requirement: Automerge document is the source of truth
All bookmark state SHALL live in one Automerge document under `ROOT.bookmarks[url_key]` with scalar fields as plain registers and `tags` as a map from tag to `true`. The document SHALL carry `ROOT.meta.schema = 1`.

#### Scenario: Concurrent tag add and remove converge add-wins
- **WHEN** replica A removes tag `x` from a bookmark while replica B concurrently adds tag `x` to the same bookmark, and the replicas merge
- **THEN** both replicas read the bookmark with tag `x` present

#### Scenario: Concurrent save of the same URL merges
- **WHEN** replica A adds `https://example.com/` with tags `{a}` and replica B concurrently adds the same URL with tags `{b}`, and the replicas merge
- **THEN** both replicas read exactly one bookmark for that URL with tags `{a, b}` and a single deterministic title

#### Scenario: Concurrent title edits converge
- **WHEN** two replicas set different titles on the same bookmark concurrently and merge
- **THEN** both replicas read the same title and no error is raised

### Requirement: Deletes are tombstones
Deleting a bookmark SHALL set `deleted = true` and never remove the entry from the document. Restoring SHALL set `deleted = false`. Adding a URL whose entry is tombstoned SHALL restore it and apply the new fields.

#### Scenario: Delete then re-add
- **WHEN** a bookmark is deleted and later the same URL is added with tags `{c}`
- **THEN** the bookmark is readable again, not deleted, with tags `{c}` unioned with its previous tags

#### Scenario: Delete survives merge with a concurrent edit
- **WHEN** replica A deletes a bookmark while replica B concurrently edits its description, and the replicas merge
- **THEN** the bookmark is deleted on both replicas and the edited description is retained in the tombstoned entry

### Requirement: Changing a URL is a move
Renaming a bookmark's URL SHALL tombstone the old key and create the new key carrying over title, description, tags and `created_ms`.

#### Scenario: Rename keeps metadata
- **WHEN** `https://a.example/` with tags `{x}` and title `T` is renamed to `https://b.example/`
- **THEN** `https://a.example/` is tombstoned and `https://b.example/` exists with tags `{x}`, title `T` and the original `created_ms`

#### Scenario: Rename onto an existing URL merges
- **WHEN** a bookmark is renamed to a URL that already exists and is not deleted
- **THEN** the target's tags become the union of both and its empty fields are filled from the source, and the source is tombstoned
