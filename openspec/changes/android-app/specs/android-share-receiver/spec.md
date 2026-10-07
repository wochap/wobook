## ADDED Requirements

### Requirement: Share target
The app SHALL register an activity for `ACTION_SEND` with `text/plain` that renders as a bottom sheet over the sending app, extracts the first URL from the shared text, uses `EXTRA_SUBJECT` as the initial title and focuses the TagEditor with the keyboard open.

#### Scenario: Share from a browser
- **WHEN** a browser shares a page URL
- **THEN** the sheet shows the host+path, the title from the subject, the TagEditor focused and tag suggestions

### Requirement: Fresh URL saves in one tap
For a URL not yet saved, the sheet SHALL fetch the description (and the title when no subject was shared) in the background and show the fetching/fetched/failed note. Save SHALL write locally with whatever metadata has arrived, never waiting on the network, then dismiss.

#### Scenario: Save before fetch completes
- **WHEN** the user taps Save while the description is still fetching
- **THEN** the bookmark is saved with the title and tags, the sheet closes, and the description stays empty

### Requirement: Already saved URL offers Update tags
For a URL that exists and is not deleted, the sheet SHALL show "Already saved on <date>", prefill the existing tags and replace Save with "Update tags", which adds the new tags without creating a duplicate. "Edit" SHALL open the full form.

#### Scenario: Update tags
- **WHEN** the shared URL exists with tags `{a}` and the user adds `b` and taps Update tags
- **THEN** the bookmark has tags `{a, b}` and exactly one record exists for the URL

### Requirement: Offline save
When no validated network is available the sheet SHALL skip fetching, show "Offline — saved on this device, syncs later" and still save.

#### Scenario: Airplane mode
- **WHEN** the device is offline and the user shares a URL and taps Save
- **THEN** the bookmark is stored locally and appears in Home

### Requirement: Post-save confirmation
After Save the activity SHALL finish and show a plain-text system toast `Saved to wobook · <tags>`. If a toast cannot be shown, the sheet SHALL display an in-sheet "Saved" state for about 600 ms before finishing.

#### Scenario: Toast shown
- **WHEN** Save succeeds with tags `react, ui library`
- **THEN** a toast reading `Saved to wobook · react, ui library` appears over the sending app

### Requirement: More opens the full form
Tapping "More" SHALL open the Add/Edit form in the main activity with the current URL, title, description and tags carried over.

#### Scenario: Continue in form
- **WHEN** the user has typed tag `a` and taps More
- **THEN** the form opens with the URL, the title and chip `a` already present
