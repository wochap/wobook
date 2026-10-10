# android-favicons Specification

## Purpose
Shows each bookmarked site's own icon on Android, cached per host on the device only, with a letter-tile fallback, an on/off setting and a manual refresh of all icons.

## Requirements

### Requirement: Icons are cached per host on the device only
The app SHALL store site icons per host (host plus non-default port) in its cache directory. A found icon SHALL be reused for 30 days and a "no icon" result SHALL be reused for 7 days before the host is fetched again. Icons and "no icon" results SHALL never be written to the bookmark model, the Automerge document or any synced data, and SHALL never be requested from a third-party icon service.

#### Scenario: Cached icon is reused
- **WHEN** a host's icon was fetched 3 days ago and a row for that host becomes visible
- **THEN** the icon is shown from the cache and no network request is made

#### Scenario: Negative result expires
- **WHEN** a host had no icon 8 days ago and a row for that host becomes visible
- **THEN** the app fetches the host again

#### Scenario: Not synced
- **WHEN** a device with cached icons syncs with a peer
- **THEN** the peer receives no icon data and its document contains no icon fields

### Requirement: Rows load icons lazily
With "Load site icons" on, a row whose host has no fresh cache entry SHALL request the icon when the row becomes visible. The app SHALL run at most 4 icon fetches at once and at most one fetch per host at a time; rows sharing a host SHALL share the result. A failed fetch SHALL be stored as "no icon".

#### Scenario: Shared host
- **WHEN** 10 visible rows belong to the same uncached host
- **THEN** exactly one fetch for that host runs and all 10 rows show its icon when it completes

#### Scenario: Concurrency cap
- **WHEN** 20 rows with 20 different uncached hosts become visible at once
- **THEN** no more than 4 fetches are in progress at any moment

### Requirement: Letter tile fallback
When a host has no icon, its icon is still loading, or "Load site icons" is off, the row SHALL show a letter tile: the first letter or digit of the host without a leading `www.`, uppercased, on a background colour chosen deterministically from the host from the theme palette. The same host SHALL always get the same colour.

#### Scenario: Host without icon
- **WHEN** `https://www.example.org/page` has no icon
- **THEN** its row shows a tile with the letter `E`

#### Scenario: Stable colour
- **WHEN** two rows have the same host
- **THEN** both tiles use the same background colour

### Requirement: Load site icons setting
Settings SHALL have a "Load site icons" toggle, on by default. While off, the app SHALL make no icon network requests, rows SHALL show letter tiles, cached icons SHALL stay on disk, and the "Refresh site icons" row SHALL be disabled. Turning it back on SHALL show cached icons immediately.

#### Scenario: Off stops network
- **WHEN** "Load site icons" is off and the user scrolls through uncached hosts
- **THEN** no icon requests are made and every row shows a letter tile

#### Scenario: Back on uses cache
- **WHEN** the setting is turned off and on again
- **THEN** previously cached icons appear without new requests

### Requirement: Refresh site icons
Settings SHALL have a "Refresh site icons" row. Its action SHALL start one background job, unique by name, that re-fetches the icon of every distinct host among live bookmarks, ignoring cache ages, with at most 4 fetches at once and only while a network connection is available. A second start while the job runs SHALL do nothing. While idle, the row subtitle SHALL read `<n> cached · last run <relative time>` (or `never run`). While running, it SHALL read `Fetching <done> / <total> sites` with a Cancel action. When the job finishes, a snackbar SHALL read `Icons updated: <found> found, <none> none`. The job SHALL keep running when the user leaves Settings.

#### Scenario: Refresh all
- **WHEN** the library has bookmarks on 3 hosts and the user taps Refresh
- **THEN** all 3 hosts are fetched even if their cache entries are fresh, and the snackbar reports the counts

#### Scenario: Progress and cancel
- **WHEN** the job is running and the user taps Cancel
- **THEN** no new fetches start, the row returns to its idle subtitle, and icons fetched so far stay cached

#### Scenario: Double start
- **WHEN** the job is running and Refresh is triggered again
- **THEN** no second job starts
