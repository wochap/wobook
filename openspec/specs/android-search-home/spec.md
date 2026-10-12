# android-search-home Specification

## Purpose
Home screen search, highlighting, tag chip filtering, result row actions, empty/no-result/syncing states.

## Requirements

### Requirement: Search field and live results
Home SHALL show a search field pinned at the top that receives focus with the keyboard open on launch, and SHALL update results on every keystroke (debounced at most 60 ms) using the FFI `search`. Matched characters in title and URL SHALL be highlighted in the primary color at weight 600.

#### Scenario: Abbreviated query
- **WHEN** the library contains `shadcn/ui` and the user types `shcn ui`
- **THEN** that bookmark is the first row and the letters `s h c n u i` in its title are highlighted

#### Scenario: Result count line
- **WHEN** results are filtered by query or chips
- **THEN** a footer line reads `<n> of <total> · filtered by <tags>`

### Requirement: Tag chips filter with AND semantics
A horizontally scrollable row of tag chips ordered by usage SHALL sit under the search field. Tapping a chip toggles it; selected chips combine with AND and stay visible while scrolled. The selection SHALL persist while navigating to Detail and back.

#### Scenario: Two chips
- **WHEN** chips `react` and `ui` are selected
- **THEN** only bookmarks carrying both tags are listed

### Requirement: Result row
Each row SHALL show a 20 dp icon slot holding the site icon of the bookmark's host, or a letter tile when no icon is available (see the `android-favicons` capability), one-line title, one-line monospace host+path, one clipped tag line with `+N` overflow, and trailing 48 dp Copy and Open buttons. A pinned bookmark's row SHALL also show a pin glyph and a subtly tinted row background so it is distinguishable at a glance, in every view including search results. Tap performs the configured tap behaviour, long press opens an action sheet (Open in browser, Copy URL, Share, Edit, Pin or Unpin, Delete), swipe right copies the URL.

#### Scenario: Copy from row
- **WHEN** the user taps the Copy button
- **THEN** the clipboard contains the bookmark URL and a brief confirmation is shown

#### Scenario: Long press sheet
- **WHEN** the user long-presses a row and taps Delete
- **THEN** the bookmark is tombstoned and a snackbar with Undo appears for 6 seconds

#### Scenario: Icon slot is never an empty square
- **WHEN** a row is shown for a host with no cached icon
- **THEN** the slot shows a letter tile, then the site icon if one is fetched

#### Scenario: Pinned row is visually distinct
- **WHEN** a pinned and an unpinned bookmark are both visible, in the unfiltered list, under a chip or in search results
- **THEN** the pinned row shows the pin glyph and the tinted background and the unpinned row shows neither

#### Scenario: Pin from the sheet
- **WHEN** the user long-presses an unpinned row and taps Pin
- **THEN** the bookmark moves to the top of the unfiltered list and its row shows the pin glyph and tinted background

#### Scenario: Unpin from the sheet
- **WHEN** the user long-presses a pinned row
- **THEN** the sheet shows Unpin instead of Pin, and tapping it removes the glyph and returns the bookmark to its recency position

### Requirement: Home states
Home SHALL render: empty library (explains share-sheet saving and pairing with Scan QR and Show my QR buttons), no results (message plus "Add a bookmark" prefilled with the query when it parses as a URL), results, idle (keyboard dismissed, Recent list, Add FAB), and syncing (2 dp indeterminate line under the search field). Offline with local data SHALL look identical to online. The Add FAB SHALL be hidden while the keyboard is visible.

#### Scenario: Empty library
- **WHEN** the library has no bookmarks
- **THEN** the empty state is shown with both pairing buttons and the Add FAB

#### Scenario: Syncing indicator
- **WHEN** sync status is `Syncing`
- **THEN** the indeterminate line is visible and the list remains interactive

#### Scenario: Keyboard hides FAB
- **WHEN** the search field is focused and the keyboard is up
- **THEN** the Add FAB is not visible

### Requirement: Pinned bookmarks lead the unfiltered list
When the search field is blank and no tag chip is selected, Home SHALL list pinned bookmarks first, ordered by `pinned_ms` descending, followed by unpinned bookmarks in the existing recency order. The existing "Recent" header stays above the list. When the search field holds text or at least one tag chip is selected, ordering SHALL be unchanged from before pinning existed (fuzzy rank for a query, recency for chips) and pin state SHALL NOT affect it.

#### Scenario: Pins above recent
- **WHEN** the library has three bookmarks, the oldest is pinned, and the search field is blank with no chips selected
- **THEN** the oldest bookmark is the first row, followed by the other two in recency order

#### Scenario: Most recently pinned first
- **WHEN** bookmark A is pinned and then bookmark B is pinned
- **THEN** B is listed above A

#### Scenario: Chip filter ignores pins
- **WHEN** the tag chip `react` is selected and the search field is blank
- **THEN** bookmarks tagged `react` are listed in recency order regardless of pin state

#### Scenario: Search ignores pins
- **WHEN** the user types a query matching an unpinned bookmark better than a pinned one
- **THEN** the unpinned bookmark is listed first
