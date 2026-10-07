# extension-save-popup Specification

## Purpose
Browser extension popup for saving/editing the active tab as a bookmark, prefilled from tab metadata.

## Requirements

### Requirement: Prefill from the active tab
When the popup opens on the Save tab, it SHALL read the active tab's URL and title and, for http(s) pages where scripting is permitted, run a one-shot script that returns `document.title`, `meta[name=description]` and `meta[property=og:description]`. The URL, title and description fields SHALL be prefilled from these values. Pages that cannot be scripted SHALL fall back to the tab title with an empty description.

#### Scenario: Normal page
- **WHEN** the popup opens on `https://ui.shadcn.com/docs` whose page has a meta description
- **THEN** the URL, title and description fields are filled without any request to the daemon other than `get` and `tags`

#### Scenario: Internal page
- **WHEN** the active tab is `about:addons` or `chrome://extensions`
- **THEN** the Save button is disabled and the form says the page cannot be bookmarked

### Requirement: Already-saved detection
The popup SHALL send `get` for the normalized tab URL. When the bookmark exists and is not deleted, the popup SHALL show a banner "Already saved on <created date>", prefill the existing tags, title and description, and relabel the primary button "Update tags", which SHALL send `update` with the tag set only.

#### Scenario: Existing bookmark
- **WHEN** the tab URL is already bookmarked with tags `{react}`
- **THEN** the banner is shown, chip `react` is present and pressing the primary button sends `update {url, tags}` and no `add`

#### Scenario: Deleted bookmark
- **WHEN** the tab URL exists only as a tombstone
- **THEN** the popup treats it as fresh and Save sends `add` with `merge: true`

### Requirement: Save sends a merge add without fetch
Save SHALL send `add` with `url`, `title`, `description`, `tags`, `fetch: false`, `merge: true` and `origin: "extension"`, show a confirmation, and close the popup after 600 ms. `Ctrl+Enter` SHALL trigger Save.

#### Scenario: Fresh save payload
- **WHEN** the user adds tags `ui library` and `react` and presses Save
- **THEN** the `add` request contains `tags: ["react","ui library"]`, `fetch: false` and `merge: true`

#### Scenario: Keyboard save
- **WHEN** focus is in the tag editor and the user presses `Ctrl+Enter`
- **THEN** Save is triggered once

### Requirement: Tag editor
The tag editor SHALL render tags as removable chips; typing then comma or Enter SHALL commit a chip; spaces SHALL be kept inside a tag; Backspace on an empty input SHALL remove the last chip; tags SHALL be lowercased and trimmed; duplicates SHALL be ignored. Suggestions SHALL come from the daemon's `tags` command (cached up to 60 s per popup session) ordered by count, with the 20 most recently used tags from extension storage floated first, filtered by the current input.

#### Scenario: Multi-word tag
- **WHEN** the user types `ui library` and presses Enter
- **THEN** one chip `ui library` is added

#### Scenario: Suggestion accepted
- **WHEN** the user types `rea` and presses Tab or clicks the suggestion `react`
- **THEN** chip `react` is added and the input is cleared

### Requirement: Error states
The popup SHALL show distinct, actionable messages for: native host not installed (`host_missing`), host crashed (`host_crashed`), daemon not running (`daemon_down`, with the hint `systemctl --user start wobookd`), hook rejection (the hook's message verbatim) and invalid URL (inline on the URL field). The form SHALL stay editable and Save SHALL be re-enabled after an error.

#### Scenario: Host missing
- **WHEN** `sendNativeMessage` fails with "Specified native messaging host not found"
- **THEN** the popup shows the native host install message and no save is attempted

#### Scenario: Hook rejected
- **WHEN** the daemon answers `hook_rejected` with message `blocked by policy`
- **THEN** the popup shows `blocked by policy` and keeps the entered fields

### Requirement: Keyboard shortcut opens the popup
The extension SHALL declare `_execute_action` with default shortcut `Alt+Shift+B` so the popup opens on the Save tab from any page.

#### Scenario: Shortcut
- **WHEN** the user presses `Alt+Shift+B`
- **THEN** the popup opens with the Save tab active and the tag input focused
