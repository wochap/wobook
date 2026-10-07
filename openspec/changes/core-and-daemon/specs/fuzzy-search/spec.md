## ADDED Requirements

### Requirement: Fuzzy matching over all text fields
Search SHALL use nucleo fuzzy matching (case-insensitive, smart normalization) over a haystack composed of title, URL, description and tags, and SHALL return hits ordered by score descending with `updated_ms` descending as tie-breaker.

#### Scenario: Abbreviated query matches
- **WHEN** the library contains `shadcn/ui` at `https://ui.shadcn.com/` and the query is `shcn ui`
- **THEN** that bookmark is the first hit

#### Scenario: Query matches a tag only
- **WHEN** a bookmark has tag `systemd` and no other field contains that word, and the query is `systemd`
- **THEN** the bookmark is returned

### Requirement: Match positions are reported
Each hit SHALL include the matched character indices within the haystack and the haystack segment boundaries so a client can highlight matched characters in the title and URL.

#### Scenario: Indices returned
- **WHEN** a query produces a hit
- **THEN** the hit carries a non-empty list of indices and each index is within the haystack length

### Requirement: Tag filtering
Search and list SHALL accept a set of tags and return only bookmarks that carry every given tag (AND semantics).

#### Scenario: Two tags
- **WHEN** filtering by tags `{react, ui}`
- **THEN** only bookmarks tagged with both `react` and `ui` are returned

### Requirement: Empty query lists recent first
An empty query SHALL return all non-deleted bookmarks ordered by `created_ms` descending.

#### Scenario: Empty query
- **WHEN** search is called with an empty string
- **THEN** every non-deleted bookmark is returned, newest first

### Requirement: Deleted bookmarks are hidden
Search and list SHALL exclude tombstoned bookmarks unless `include_deleted` is set.

#### Scenario: Deleted excluded by default
- **WHEN** a bookmark is deleted and search runs with its exact title
- **THEN** it is not returned, and it is returned when `include_deleted` is true
