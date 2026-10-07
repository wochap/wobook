# extension-badge Specification

## Purpose
Toolbar badge showing whether the active tab's URL is bookmarked, refreshed on tab and bookmark changes.

## Requirements

### Requirement: Badge reflects bookmarked state of the active tab
The background worker SHALL, on tab activation and on tab URL change to a completed http(s) page, send `get` for the tab URL and set a check badge on the toolbar icon for that tab when the bookmark exists and is not deleted, otherwise clear the badge.

#### Scenario: Bookmarked tab
- **WHEN** the user switches to a tab whose URL is bookmarked
- **THEN** the action badge for that tab shows a check mark

#### Scenario: Unbookmarked tab
- **WHEN** the user navigates that tab to an unbookmarked URL
- **THEN** the badge is cleared

### Requirement: Badge results are cached per tab
The worker SHALL cache `{url, bookmarked}` per tab id, reuse it while the tab URL is unchanged, drop it when the tab closes, and refresh it when the popup reports a save.

#### Scenario: Save refreshes badge
- **WHEN** the popup saves the current tab's URL
- **THEN** the badge shows a check without waiting for a tab change

#### Scenario: Tab closed
- **WHEN** a tab is removed
- **THEN** its cache entry is deleted

### Requirement: Badge failures are silent
Any host or daemon error during a badge lookup SHALL clear the badge and SHALL NOT surface a notification or console error at error level.

#### Scenario: Daemon down
- **WHEN** the daemon is not running and the user switches tabs
- **THEN** no badge is shown and no user-visible error appears
