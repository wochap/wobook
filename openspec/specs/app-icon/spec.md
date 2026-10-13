# app-icon Specification

## Purpose

Defines the wobook app icon artwork (the "Synced pair" design) and requires that the Android adaptive icon and the browser extension's icon set both render it consistently.

## Requirements

### Requirement: Synced pair artwork
The app icon SHALL depict the "Synced pair" design: two offset bookmark-ribbon outlines (a lavender ribbon behind a blue filled ribbon) on a 108x108 canvas, matching the artwork recorded in `design/project/App Icons.dc.html`. The icon SHALL render inside a circle mask.

#### Scenario: Icon matches the design doc
- **WHEN** the Android launcher icon or an extension icon PNG is rendered
- **THEN** it shows two offset ribbon shapes (not a single ribbon), circle-masked, matching the paths in `design/project/App Icons.dc.html`

### Requirement: Android adaptive icon themed variants
The Android adaptive icon SHALL provide a foreground, a background, and a monochrome (Android 13+ themed-icon) variant, all using the "Synced pair" artwork, and SHALL render correctly under both the Catppuccin Mocha (dark) and Latte (light) system themes.

#### Scenario: Dark system theme
- **WHEN** the device is in dark mode and the user views the wobook launcher icon
- **THEN** the icon background is the Catppuccin Mocha mantle color and the two ribbons are visible in their designed colors

#### Scenario: Light system theme
- **WHEN** the device is in light mode and the user views the wobook launcher icon
- **THEN** the icon background is the Catppuccin Latte base color and the two ribbons remain legible against it

#### Scenario: Themed (monochrome) icon
- **WHEN** the device has Android 13+ themed icons enabled
- **THEN** the wobook launcher icon renders as a single-color silhouette of the two-ribbon "Synced pair" shape

### Requirement: Browser extension icon set
`extension/icons/` SHALL provide the "Synced pair" artwork at 16, 32, 48 and 128 px, plus monochrome `mono-16`, `mono-32`, `mono-48` and `mono-128` variants, matching the shapes used by the Android icon, and `extension/manifest.json` SHALL continue to reference them at their existing sizes.

#### Scenario: Toolbar icon matches Android icon
- **WHEN** a user compares the browser toolbar icon to the Android launcher icon
- **THEN** both show the same two-ribbon "Synced pair" shape
