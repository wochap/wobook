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
Each row SHALL show an optional 20 dp favicon slot (empty surface square by default), one-line title, one-line monospace host+path, one clipped tag line with `+N` overflow, and trailing 48 dp Copy and Open buttons. Tap performs the configured tap behaviour, long press opens an action sheet (Open in browser, Copy URL, Share, Edit, Delete), swipe right copies the URL.

#### Scenario: Copy from row
- **WHEN** the user taps the Copy button
- **THEN** the clipboard contains the bookmark URL and a brief confirmation is shown

#### Scenario: Long press sheet
- **WHEN** the user long-presses a row and taps Delete
- **THEN** the bookmark is tombstoned and a snackbar with Undo appears for 6 seconds

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
