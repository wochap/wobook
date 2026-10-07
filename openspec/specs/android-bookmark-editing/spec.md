# android-bookmark-editing Specification

## Purpose
Detail screen, Add/Edit form with TagEditor and fetch states, URL change as rename, delete with undo.

## Requirements

### Requirement: Detail screen
Detail SHALL show title, full selectable URL in monospace, description, tag chips, saved date and "Last changed on <device>" line, with primary actions Open, Copy, Share and secondary Edit and Delete in the app bar.

#### Scenario: Open from detail
- **WHEN** the user taps Open
- **THEN** an `ACTION_VIEW` intent for the URL is launched

### Requirement: Add and Edit share one form
The form SHALL have URL, Title, Description and Tags fields with Save in the app bar and a Save button kept above the keyboard. In Add mode, pasting a URL SHALL trigger `fetch_metadata` (when auto-fetch is on) with shimmer placeholders on Title and Description, then a one-line fetched/failed/offline note; failure keeps fields editable with a Retry. If the URL already exists, the form SHALL switch to Edit with the banner "Already saved on <date>, editing it".

#### Scenario: Fetch fills fields
- **WHEN** the user pastes a URL whose page has a title and description and auto-fetch is on
- **THEN** Title and Description shimmer, then fill, and the note reads "Filled from the page · edit anything"

#### Scenario: Fetch failed
- **WHEN** the fetch fails
- **THEN** the note shows a warning with Retry and Save remains enabled

#### Scenario: Existing URL switches to edit
- **WHEN** the pasted URL is already saved
- **THEN** the banner appears and Save performs an update instead of creating a duplicate

### Requirement: TagEditor
The Tags field SHALL be a chip editor: comma or Enter/IME action commits a chip, space is part of the tag, pasted text splits on commas, backspace on empty input re-opens the last chip, and suggestions from existing tags (most used first, max 8, matched case-insensitively across the whole tag) appear below the field. Committed chips SHALL be normalized through the FFI `parse_tags`.

#### Scenario: Multi-word tag
- **WHEN** the user types `ui library` and presses Enter
- **THEN** a single chip `ui library` is created

#### Scenario: Suggestion across space
- **WHEN** the user types `ui lib`
- **THEN** `ui library` is offered as a suggestion

### Requirement: URL is read-only in Edit until Change
In Edit mode the URL field SHALL be read-only with a "Change" button. Tapping it SHALL reveal an editable field and the note "Changing the URL replaces this bookmark; tags and description are kept." Saving with a changed URL SHALL call `rename`.

#### Scenario: Rename through the form
- **WHEN** the user changes the URL and saves
- **THEN** the old URL is tombstoned, the new URL exists with the same tags and description, and the user lands on the new Detail

### Requirement: Delete with undo
Delete from Detail, the action sheet or the form SHALL tombstone immediately, return to Home and show a snackbar "Deleted “<title>”" with Undo for 6 seconds. Undo SHALL call `restore`. No confirmation dialog SHALL be shown for delete.

#### Scenario: Undo restores
- **WHEN** the user deletes a bookmark and taps Undo within 6 seconds
- **THEN** the bookmark is listed again with its previous tags
