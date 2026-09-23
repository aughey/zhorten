# zhorten-service

`zhorten-service` is the transport-agnostic implementation of zhorten's functional API. It defines the operations the application supports without depending on HTTP, Axum, sled, or the browser.

## Responsibilities

- Define the `Database` trait required by the service functions.
- Implement link listing, creation, deletion, and redirect lookup as plain Rust functions.
- Validate short codes using `zhorten-core::ValidCode`.
- Validate destination URLs and allow only `http://` and `https://` links.
- Return domain-level errors that adapters can translate into transport-specific responses.

## API Shape

The public service functions are:

- `list_links(database)` returns dashboard data.
- `create_link(database, code, url, created_at)` validates input and inserts a new link through the database trait.
- `remove_link(database, code)` validates the code and removes the link through the database trait.
- `follow_link(database, code, clicked_at)` validates the code, records the click through the database trait, and returns the destination URL.

Most work is delegated to the `Database` trait. The service layer adds the functional rules that should be independent of any transport adapter: valid codes, valid URL syntax, supported URL schemes, conflict reporting, and not-found reporting.

## Boundaries

This crate should not contain HTTP routing, status-code mapping, JSON extraction, session handling, or sled-specific persistence code. Those concerns belong in `zhorten-server`.

