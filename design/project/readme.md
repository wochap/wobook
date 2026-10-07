# wobook — Android design handoff

Files: `wobook.dc.html` (all frames + design system + flow overview, canvas), `wobook-tokens.css` (the only stylesheet; variables only). Frames are 412×915 dp; every value in a frame is a `var(--wb-*)`.

Palette: Catppuccin Mocha (dark, default) and Latte (light). Switch with `data-theme="light"`. Accent = Catppuccin Blue (`#89b4fa` / `#1e66f5`), used only for lines, outlines, matched characters, active chips and the sync line — never large fills. Type: Inter; JetBrains Mono for URLs and fingerprints. Icons: Phosphor Regular (24dp app bars/rows, 22 trailing row actions, 16 inside chips); Fill only for an active state.

## Tokens → Compose

colorScheme (dark values; light are the Latte equivalents)
- background, surface ← `--wb-bg` (base #1e1e2e)
- surfaceContainerLow ← `--wb-bg-low` (mantle) — bottom sheets, menus, keyboard ground
- surfaceContainerLowest ← `--wb-bg-lowest` (crust)
- surfaceContainer ← `--wb-surface` (surface0) — chips, search field, fingerprint box, favicon slot
- surfaceContainerHigh ← `--wb-surface-high` (surface1) — pressed row, key caps
- outline ← `--wb-outline` (surface2); outlineVariant ← `--wb-outline-variant` (surface1) — field borders, dividers
- onSurface ← `--wb-text`; onSurfaceVariant ← `--wb-text-muted`; disabled/hint ← `--wb-text-faint`
- primary ← `--wb-accent`; onPrimary ← `--wb-on-accent` (crust); primaryContainer ← `--wb-accent-container` (accent @16% alpha)
- error ← `--wb-error` (red). Semantic extras (not in colorScheme, add a `WobookColors` CompositionLocal): success/LAN = green, tailscale = lavender, warning = yellow.
- scrim ← `--wb-scrim`; inverseSurface/inverseOnSurface ← `--wb-inverse-*` (snackbar/toast)

typography
- headlineMedium ← `--wb-t-display` 28/34 Medium
- titleLarge ← `--wb-t-title` 20/26 Medium; titleMedium ← `--wb-t-title-sm` 16/22 Medium
- bodyLarge ← `--wb-t-body-lg` 16/24 (result title, field values); bodyMedium ← `--wb-t-body` 14/20
- labelLarge ← `--wb-t-body-md` 14/20 Medium (buttons, 32dp chips); labelMedium ← `--wb-t-label` 12/16 (meta, tiny chips)
- mono ← `--wb-t-mono` 13/18 JetBrains Mono (url host+path); fingerprint 18/26 Medium mono, letterSpacing .04em
- Dynamic type to 130%: result row uses `min-height 64dp` not fixed height; title and url each 1 line ellipsised; tag row is a single clipped line with `+N`.

shapes: extraSmall 4 (`--wb-r-sm`, tiny chips), small 8 (`--wb-r-md`, buttons/fields/menus), medium 12 (`--wb-r-lg`, search field, FAB), large 20 (`--wb-r-xl`, sheets, dialog), full (filter/editor chips).
spacing: 4/8/12/16/20/24/32. Tap targets ≥48dp (`--wb-tap`). Row 64, chip 32, field 52, app bar 56.
elevation: `--wb-shadow-1/2/3` = hairline (surface0/1/2) + ambient darkness; in Compose use tonal surface colors above plus a 1dp border, not shadow elevation, on dark.

## Components

Stock Material 3 (themed through colorScheme/typography/shapes only):
TopAppBar (small), IconButton, OutlinedButton (primary = primary-colored outline, secondary = outline color, destructive = error), TextButton, Switch, ModalBottomSheet, DropdownMenu, AlertDialog (revoke only), Snackbar (undo), LinearProgressIndicator (2dp, indeterminate, for sync/connecting), SegmentedButton (Settings › tap behaviour), ExtendedFloatingActionButton (Add — outlined look: container = background, 1dp primary border).

Custom composables:
- `TagChip` — states: default (surfaceContainer, onSurfaceVariant), selected (primaryContainer + 1dp primary + check icon), removable (trailing ×, 28dp in editors), overflow `+N` (outline border), tiny 20dp row variant.
- `SearchField` — 52dp, radius 12, surfaceContainer, 1dp primary border when focused, leading magnifier, trailing clear/settings.
- `HighlightedText` — fuzzy-matched characters in primary, weight 600.
- `ResultRow` — favicon slot 20dp (empty = surfaceContainer square), title / mono url / tag line, trailing Copy (onSurfaceVariant) and Open (primary) 48dp buttons; swipe-right = copy URL; long-press = action sheet.
- `TagEditor` — chip list + inline text. **Comma or Enter (IME action) commits a chip; space is part of the tag** (`ui library`, `ai agent`). Autocomplete is fuzzy across the whole tag incl. spaces (`ui lib` → `ui library`); outlined suggestion chips with the matched prefix highlighted, in a horizontal row beneath. Helper line: “Comma or Enter adds a tag · spaces are allowed”.
- `FetchStateNote` — one 12sp line: fetching (circle-notch, primary) / fetched (check, success) / failed (warning-circle, warning + Retry) / offline (cloud-slash).
- `ShimmerField` — label stays, value row is a 14dp bar shimmering surfaceContainer→High.
- `Fingerprint` — 4×8 hex in a 2×2 grid, mono 18/26 Medium, surfaceContainer box.
- `UrlField` (edit mode) — URL is the identity. In Edit it renders read-only (surfaceContainer, muted mono) with a trailing “Change” TextButton; tapping reveals an editable OutlinedTextField plus the note “Changing the URL replaces this bookmark; tags and description are kept.” Saving writes a tombstone for the old URL and a new record.
- `DeviceRow` — first row is always this device (name, “This device · Android”, device-mobile icon, no menu), then a divider, then peers (platform icon, reachability dot, last synced, overflow menu: Rename / Revoke…). Footer carries only the sync status.
- `QrTile` — modules in crust on a text-colored (light) tile, 8–10dp quiet zone, radius 8; optional 3dp countdown ring (primary over surfaceContainer, 120 s).
- `ScanFrame` — 260dp square, 36dp corner brackets 3dp primary.
- `ReachabilityDot` — 8dp: success (LAN), tailscale (lavender), outline (unreachable).
- `SyncStatusFooter` — icon + 12sp label: Up to date / Syncing with <device> / No device reachable.
- `EmptyState` — title + body + icon-led rows, no illustration.
- `ShareReceiverSheet` — ModalBottomSheet content: header (title, mono url, More), FetchStateNote, TagEditor on `background`, suggestions row, 48dp Save; keyboard always up.

Behaviour notes: offline-with-local-data renders identically to online. Delete never confirms (snackbar undo, 6 s); revoke always confirms. Share receiver dismisses itself after Save and shows a **plain-text system `Toast`** over the host app (Android 12+: text only, no icon, no custom colors): “Saved to wobook · <tags>”. If a toast can’t be shown (activity already finished before `Toast.show`, notifications disabled), fall back to an in-sheet confirmation: the Save button swaps to a success-outlined “✓ Saved” for 600 ms, then the sheet dismisses.

Large screens: single column; content clamps to 640dp and centers (`Modifier.widthIn(max = 640.dp)`), gutters are plain background. Dynamic type 1.3×: all type tokens are `sp`; rows use min-height, tiny tag chips grow to 24dp, trailing 48dp buttons don’t scale.
