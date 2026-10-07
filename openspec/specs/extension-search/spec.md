# extension-search Specification

## Purpose
Browser extension fuzzy search over bookmarks via the daemon, opening results in tabs.

## Requirements

### Requirement: Fuzzy search through the daemon
The Search tab SHALL send `search {query, limit: 50}` to the daemon, debounced by 80 ms, and render each hit with title, host and path in monospace, and tags as tiny chips. An empty query SHALL show the 20 most recent bookmarks.

#### Scenario: Abbreviated query
- **WHEN** the user types `shcn ui` and the library contains `shadcn/ui`
- **THEN** that bookmark is rendered first

#### Scenario: No results
- **WHEN** the query matches nothing
- **THEN** the list shows "Nothing matches" and no error

### Requirement: Matched characters are highlighted
The popup SHALL use the `indices` and haystack segment boundaries returned by the daemon to highlight matched characters in the title and URL only, using the accent color.

#### Scenario: Highlight spans
- **WHEN** a hit returns indices covering characters in the title and the URL
- **THEN** those characters are wrapped in highlight spans and characters in the description or tags are not

### Requirement: Open and copy actions
Enter or clicking the open icon SHALL open the selected bookmark in a new tab and close the popup. Clicking the row or pressing `Ctrl+C` SHALL copy the URL to the clipboard and show "Copied" for 1 second. Up and Down SHALL move the selection.

#### Scenario: Open in new tab
- **WHEN** the second hit is selected with Down and Enter is pressed
- **THEN** `tabs.create` is called with that bookmark's URL and the popup closes

#### Scenario: Copy
- **WHEN** the user clicks a result row
- **THEN** the URL is written to the clipboard and "Copied" is shown briefly

### Requirement: Search errors reuse the save error states
Daemon or host failures during search SHALL display the same `host_missing`, `host_crashed` and `daemon_down` messages as the Save tab.

#### Scenario: Daemon down during search
- **WHEN** the host returns `daemon_unavailable`
- **THEN** the Search tab shows the daemon not running message instead of results
