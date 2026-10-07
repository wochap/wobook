# Claude Design prompt: wobook Android app

Paste everything below the line into Claude Design (claude.ai/design). Fill the
`INPUTS` block first. Export the handoff bundle into `android/design/` when done.

---

You are designing the Android app for **wobook**, a personal bookmark manager.
Produce high-fidelity phone mockups (HTML/CSS prototypes) for every screen and
state listed, plus a small design system, ready for a coding agent to rebuild
pixel-perfect in Jetpack Compose (Material 3 components are allowed but the
look must come from this design system, not stock Material).

## INPUTS

```
APP_NAME:        wobook
DESIGN_SYSTEM:   new            # "new" = create one; or "nocturne" = reuse the
                                # attached Nocturne system (dark blue-grey, Inter,
                                # 8px radii, outlined buttons, Phosphor icons)
THEME:           dark + light   # both required; dark is the primary
ICONS:           Phosphor
FONT:            Inter (or system default if unavailable)
DEVICE_FRAME:    412 x 915 dp (Pixel-class), gesture nav, status bar visible
DENSITY:         compact, list-first, information dense but calm
```

## Product context

wobook replaces `buku` (a CLI bookmark manager). A bookmark is only:

- `url` (identity, unique)
- `title`
- `description`
- `tags` (set of lowercase words, e.g. `linux`, `security`, `ui library`)
- `created` time, and a `deleted` tombstone

There is no cloud. Devices (Linux desktops and this Android app) sync
peer-to-peer over LAN or Tailscale using a CRDT, so the app works fully
offline and every write succeeds locally. Users pair devices by scanning a QR
code. Typical library: 100 to 2,000 bookmarks. One user, personal use.

Primary jobs on the phone, in order of frequency:

1. Save the URL currently in the browser via the Android share sheet, add tags,
   done in under 5 seconds.
2. Find a bookmark by fuzzy search (like `fzf`: typing `shcn ui` matches
   `shadcn/ui`), then open it in the browser or copy the URL.
3. Edit tags/title/description, or delete.
4. Rarely: pair a new device, review paired devices, check sync status, import
   or export.

## Screens

Design each as a full phone frame. Include every listed state.

### 1. Search / Home (default screen)

- Search field pinned at top, keyboard opens on launch, hint "Search
  bookmarks". Results update as you type. Fuzzy matches are highlighted at the
  matched characters, per row.
- Below the field: a horizontally scrollable row of tag chips (most used
  first). Tapping a chip filters; multiple chips AND together. Active chips
  stay visible when scrolled.
- Result row: title (1 line), url host + path (1 line, muted), tags as tiny
  chips (wrap to max 1 line, overflow "+3"). No thumbnails, no favicons needed
  (optional small favicon slot, must look fine when empty).
- Tap row = open detail. Long press = contextual action sheet: Open in browser,
  Copy URL, Share, Edit, Delete.
- Quick actions on the row (swipe or trailing icon): Copy URL. Opening in the
  browser should also be reachable in one tap from the row.
- FAB or bottom action: "Add" (manual URL entry).
- States: empty library (first launch, explain share-sheet saving and
  pairing), no results, results, loading/sync in progress indicator (subtle,
  e.g. a thin line or small icon in the top bar, never a blocking spinner),
  offline but local data present (should look identical to online; sync
  status lives in Settings).

### 2. Bookmark detail

- Title, full URL (selectable, monospace or muted), description, tags, created
  date, "last changed on <device name>" line.
- Primary actions: Open, Copy URL, Share. Secondary: Edit, Delete (with undo
  snackbar, no confirmation dialog).

### 3. Add / Edit bookmark (same form, two entry points)

- Fields: URL, Title, Description, Tags.
- Tags input: chip editor. Typing shows autocomplete from existing tags.
  Space or comma commits a chip. Recent/frequent tags offered as tappable
  suggestions under the field.
- Title and description auto-fill after the URL is pasted (fetched from the
  page). Show the fetch state: fetching (shimmer on the two fields), fetched,
  failed (fields stay editable, small inline note, never a dialog).
- URL already saved: form switches to "editing existing bookmark" with a
  banner "Already saved on <date>, editing it".
- Save in the top bar and as a bottom button reachable with the keyboard up.

### 4. Share-sheet receiver (the most important flow)

The user shares a link from Chrome. The app opens as a bottom sheet or a
compact dialog over the browser, not a full screen:

- URL prefilled, title prefilled by the browser share text, description
  fetched in the background.
- Tag chip editor focused with keyboard up, suggestions visible.
- One tap "Save". After save: short confirmation, sheet dismisses, user is back
  in the browser.
- "More" expands to the full Add form (screen 3).
- States: fresh URL, URL already saved (offer "Update tags" instead of a
  duplicate), fetch failed, offline.

### 5. Pairing

Pairing is symmetric and uses a QR payload that contains the device's public
key fingerprint, network endpoints, and a one-time secret valid for 120
seconds.

- 5a Scan: full-screen camera with a framing square, torch toggle, "Paste code
  instead" fallback for a JSON string, and a cancel. On decode: a short
  "connecting" state.
- 5b Show my QR: large QR, the device name, a 120 s countdown ring, "Copy as
  text" button. Used for Android-to-Android pairing.
- 5c Confirm: both devices show the same fingerprint (4 groups of 8 hex
  characters, e.g. `3fa1c9d2 77be0041 9e2c5a10 b4d7f6e3`) and the peer's
  device name. Buttons "Trust" / "Reject". Explain in one line why to compare
  the code.
- States: success (returns to Devices with the new peer syncing), expired code,
  rejected on the other side, network unreachable, camera permission denied.

### 6. Devices

- List of paired devices: name, platform icon (Linux / Android), last synced
  "2 min ago", reachability dot (LAN, Tailscale, unreachable).
- Row actions: rename, revoke (with confirmation, this one is destructive and
  not undoable).
- Buttons: "Scan QR", "Show my QR".
- Sync status footer: "Up to date", "Syncing with gdesktop", "No device
  reachable, changes saved locally".

### 7. Settings

- This device name (editable).
- Sync: background sync toggle (periodic on Wi-Fi), "Sync now".
- Data: Import (JSONL, Netscape HTML, buku database), Export (JSONL, Netscape
  HTML), show library size and last change.
- Behaviour: default action on row tap (Open vs Detail), auto-fetch title and
  description toggle.
- About: version, licenses.

### 8. Onboarding (first launch only, max 2 screens)

- Name this device.
- Choose: "Pair with an existing device" (goes to Scan) or "Start fresh".
  Then land on the empty Home.

## Components to specify

Tag chip (default, selected, removable in editor, overflow "+N"), search field
with match highlighting, result row, bottom action sheet, snackbar with undo,
fingerprint display, QR frame, sync status indicator, empty states, dialogs
(only for destructive revoke), bottom sheet for the share receiver, form
fields with fetch-shimmer state.

## Design direction

- Fast and quiet. This is a tool opened 20 times a day for 5 seconds. No
  illustrations beyond empty states, no onboarding carousels, no gradients as
  decoration.
- Dense list, generous tap targets (min 48 dp), one accent color used as lines,
  highlights and the matched-character emphasis, never as large fills.
- Dark theme first, light theme derived from the same tokens. Follows system
  theme.
- Keyboard-up layouts must be designed, not assumed: Home, Add/Edit and the
  share sheet spend most of their life with the keyboard open.
- Respect Android conventions: predictive back, edge-to-edge, system share
  sheet, Material 3 motion is fine. No iOS patterns.
- Accessibility: contrast at least 4.5:1 for text, 3:1 for chrome and the
  accent on ground. Dynamic type up to 130 percent must not break the result
  row.

## Deliverables

1. Design system: tokens (color ramps for dark and light, type scale, spacing,
   radii, elevation), components above with all states, icon usage notes. One
   stylesheet, variables only, no hard-coded values in screens.
2. One HTML prototype per screen and state listed, in phone frames, named
   `Home-*.html`, `Detail-*.html`, `Form-*.html`, `Share-*.html`, `Pair-*.html`,
   `Devices-*.html`, `Settings.html`, `Onboarding-*.html`.
3. A flow overview page linking the frames in order of the two main journeys:
   share-sheet save, and search-open.
4. A short `readme.md` for the coding agent: what each token maps to in
   Compose (MaterialTheme colorScheme slots, typography slots), which
   components are stock Material 3 and which are custom.

Ask before designing if anything about the sync/pairing model is unclear. Do
not invent features beyond this list (no folders, no favorites, no sharing
between users, no accounts).
