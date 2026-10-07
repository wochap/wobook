## ADDED Requirements

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
