# metadata-fetch Specification

## Purpose
Fetch title and description for a URL with timeouts, size limits, failure tolerance, and an opt-out.

## Requirements

### Requirement: Fetch title and description on add
When a bookmark is added without a title and fetching is not disabled, the system SHALL fetch the page over HTTPS/HTTP with rustls, follow at most 5 redirects, time out after 8 seconds, read at most 512 KiB, and extract `<title>` (fallback `og:title`) and `<meta name="description">` (fallback `og:description`).

#### Scenario: Successful fetch fills fields
- **WHEN** a URL serving HTML with `<title>Hello</title>` and a description meta tag is added without a title
- **THEN** the stored bookmark has title `Hello` and the meta description

#### Scenario: Title supplied skips fetch
- **WHEN** a bookmark is added with `--title`
- **THEN** no HTTP request is made

#### Scenario: Fetch disabled
- **WHEN** a bookmark is added with `fetch: false`
- **THEN** no HTTP request is made and title and description stay as supplied or empty

### Requirement: Fetch failures never block saving
Any fetch error (timeout, non-HTML content type, HTTP error, DNS failure, body too large) SHALL leave title and description empty and the add SHALL still succeed. The response SHALL indicate that the fetch failed.

#### Scenario: Unreachable host
- **WHEN** the URL's host does not resolve
- **THEN** the bookmark is saved with empty title and the response contains `fetch: "failed"`

#### Scenario: Non-HTML resource
- **WHEN** the URL returns `Content-Type: application/pdf`
- **THEN** the body is not downloaded beyond the headers and the bookmark is saved with empty title

### Requirement: Extracted text is sanitized
Extracted title and description SHALL have whitespace collapsed to single spaces, be trimmed, be decoded from the declared charset, and be capped at 512 and 4096 characters respectively.

#### Scenario: Multi-line title
- **WHEN** the page title contains newlines and repeated spaces
- **THEN** the stored title is a single line with single spaces

### Requirement: Favicon discovery for a site origin
Given a site origin (scheme, host, optional port), the system SHALL fetch the origin's root page with the same timeout, redirect and body limits as the title fetch, and collect `<link>` elements whose `rel` contains `icon` (including `shortcut icon`) or `apple-touch-icon`, resolving each `href` against the final page URL. It SHALL skip SVG icons (`type="image/svg+xml"` or a `.svg` path), prefer the declared size closest to 48 px (entries without `sizes` rank after sized ones), and fall back to `<origin>/favicon.ico` when no candidate is usable or the page cannot be loaded. The icon download SHALL be capped at 256 KiB and SHALL be accepted only when its `Content-Type` is `image/*` or its bytes start with a PNG, ICO, GIF, JPEG or WebP signature. The result SHALL be the image bytes with their media type, or no icon. Failures SHALL never be fatal to the caller.

#### Scenario: Sized link wins
- **WHEN** the root page declares icons of sizes `16x16`, `32x32` and `192x192`
- **THEN** the `32x32` icon is downloaded

#### Scenario: SVG skipped
- **WHEN** the only declared icon is `icon.svg`
- **THEN** `/favicon.ico` is tried instead

#### Scenario: Relative href after redirect
- **WHEN** `https://a.example/` redirects to `https://www.a.example/home/` declaring `href="img/fav.png"`
- **THEN** the icon is fetched from `https://www.a.example/home/img/fav.png`

#### Scenario: Not an image
- **WHEN** the chosen icon URL returns `text/html` content without an image signature
- **THEN** the result is no icon

#### Scenario: Oversized icon
- **WHEN** the icon is larger than 256 KiB
- **THEN** the result is no icon
