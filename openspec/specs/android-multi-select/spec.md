# android-multi-select Specification

## Purpose

Lets the Android user select several bookmarks on Home and delete, pin, unpin or re-tag them in one action instead of one bookmark at a time.

## Requirements

### Requirement: Entering and leaving selection mode
Long-pressing a Home result row SHALL enter selection mode with that row selected. While in selection mode a tap on a row SHALL toggle its selection instead of performing the tap behaviour, and a long press SHALL also toggle it. Selection mode SHALL end when the user taps Close in the selection bar, presses system Back, completes Delete, or when the selection becomes empty. Leaving selection mode SHALL clear the selection and restore the search row with the query and chips it had before.

#### Scenario: Long press starts selection
- **WHEN** the user long-presses a row on Home
- **THEN** selection mode starts, that row shows as selected and the selection bar reads `1 selected`

#### Scenario: Tap toggles while selecting
- **WHEN** selection mode is active and the user taps an unselected row, then taps it again
- **THEN** the row becomes selected and then unselected, and no Detail screen or browser is opened

#### Scenario: Back leaves selection
- **WHEN** selection mode is active and the user presses system Back
- **THEN** selection mode ends, no row is selected and the app stays on Home

#### Scenario: Deselecting the last row leaves selection
- **WHEN** exactly one row is selected and the user taps it
- **THEN** selection mode ends

### Requirement: Selection bar and frozen list
While in selection mode a selection bar SHALL replace the search row, showing Close, the count `<n> selected`, and the actions Select all, Pin or Unpin, Tags, Delete and an overflow menu. The query and selected tag chips SHALL NOT change while selecting, so the listed rows stay the same apart from sync updates. Swipe-to-copy and the per-row Copy and Open buttons SHALL be disabled while selecting, and selected rows SHALL be visibly marked. The Add FAB SHALL be hidden while selecting.

#### Scenario: Row buttons inert while selecting
- **WHEN** selection mode is active
- **THEN** rows show no active Copy or Open buttons, a right swipe does not copy, and the Add FAB is not visible

#### Scenario: Filter is kept
- **WHEN** the user has typed `linux`, long-presses a row and then leaves selection mode
- **THEN** the search field still reads `linux` and the same results are listed

### Requirement: Select all
Select all SHALL select every row in the current list, which is the filtered list when a query or chips are active. When every listed row is already selected, the control SHALL read Deselect all and SHALL clear the selection, which ends selection mode.

#### Scenario: Select all under a chip
- **WHEN** chip `linux` is selected, two bookmarks carry it, and the user long-presses one row and taps Select all
- **THEN** the selection bar reads `2 selected` and bookmarks without `linux` are not selected

### Requirement: Single-selection overflow actions
When exactly one row is selected, the overflow menu SHALL offer Open in browser, Copy URL, Share and Edit for that bookmark. When more than one row is selected those four entries SHALL NOT be offered. Choosing Open, Copy URL or Share SHALL leave selection mode; choosing Edit SHALL leave selection mode and open the Edit form.

#### Scenario: Edit one bookmark
- **WHEN** one row is selected and the user opens the overflow menu and taps Edit
- **THEN** the Edit form opens for that bookmark

#### Scenario: No single actions for many
- **WHEN** two rows are selected and the user opens the overflow menu
- **THEN** Open in browser, Copy URL, Share and Edit are not offered

### Requirement: Bulk pin and unpin
The pin action SHALL read Pin when at least one selected bookmark is unpinned and SHALL then pin every selected bookmark that is not pinned. It SHALL read Unpin when every selected bookmark is pinned and SHALL then unpin all of them. After the action selection mode SHALL end. Bulk pin and unpin SHALL NOT offer Undo.

#### Scenario: Mixed selection pins all
- **WHEN** one pinned and two unpinned bookmarks are selected and the user taps Pin
- **THEN** all three are pinned, the unfiltered list shows them above unpinned bookmarks, and the earlier pin keeps its position relative to the new ones

#### Scenario: All pinned shows Unpin
- **WHEN** only pinned bookmarks are selected
- **THEN** the action reads Unpin and tapping it unpins all of them

### Requirement: Bulk tags sheet
Tags SHALL open a sheet listing every tag carried by at least one selected bookmark, each with a state and a count: all (every selected bookmark has it), some (`<k> of <n>`), or none. Tapping a tag SHALL cycle its state; a tag that started as some cycles some, all, none, some; any other tag toggles between all and none. A tag input with the same commit, normalization and suggestion rules as the form's TagEditor SHALL add new tags in the all state. Apply SHALL add each tag in the all state to the selected bookmarks missing it, remove each tag in the none state from the selected bookmarks carrying it, and leave tags in the some state untouched; it SHALL NOT change titles, descriptions or any other tag. Cancel or dismissing the sheet SHALL change nothing. After Apply selection mode SHALL end. Bulk tag changes SHALL NOT offer Undo.

#### Scenario: Partial tag left alone
- **WHEN** two bookmarks are selected, only one has `cli`, and the user opens Tags and taps Apply without changes
- **THEN** no bookmark is modified and `cli` stays on only that one

#### Scenario: Add a tag to all
- **WHEN** two bookmarks are selected and the user types `to read` and Enter in the sheet and taps Apply
- **THEN** both bookmarks carry `to read` and keep all their other tags

#### Scenario: Remove a tag from all that have it
- **WHEN** two bookmarks are selected, both have `linux`, and the user taps `linux` until it shows none and taps Apply
- **THEN** neither bookmark carries `linux` and their other tags are unchanged

#### Scenario: Promote a partial tag
- **WHEN** two bookmarks are selected, only one has `cli`, and the user taps `cli` once (some to all) and taps Apply
- **THEN** both bookmarks carry `cli`

### Requirement: Bulk delete with undo
Delete SHALL tombstone every selected bookmark without a confirmation dialog, end selection mode and show one snackbar `Deleted <n> bookmarks` with Undo for 6 seconds; with exactly one bookmark it SHALL use the single-delete wording `Deleted “<title>”`. Undo SHALL restore every bookmark that the action deleted. If some deletions fail, the snackbar SHALL read `Deleted <k> of <n> bookmarks` and Undo SHALL restore only the deleted ones.

#### Scenario: Delete two and undo
- **WHEN** two bookmarks are selected, the user taps Delete and then Undo within 6 seconds
- **THEN** both bookmarks are listed again with their previous tags

#### Scenario: Delete two
- **WHEN** two bookmarks are selected and the user taps Delete
- **THEN** neither bookmark is listed and the snackbar reads `Deleted 2 bookmarks`

### Requirement: Selection follows library changes
If a selected bookmark stops being listed because it was deleted or changed by sync, it SHALL be removed from the selection and the count SHALL update. If that empties the selection, selection mode SHALL end.

#### Scenario: Synced delete drops from selection
- **WHEN** two bookmarks are selected and one is deleted on another device and synced
- **THEN** the selection bar reads `1 selected`
