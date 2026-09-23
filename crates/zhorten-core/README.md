# zhorten-core

`zhorten-core` contains datatypes shared across zhorten crates. It is intentionally small and dependency-light so both the browser app and server-side crates can use the same API shapes without pulling in transport, storage, or UI concerns.

## Responsibilities

- Define route-safe short-link codes through `ValidCode`.
- Define shared JSON request and response payloads under `api`.
- Keep serialization and validation rules consistent between the Leptos app, service layer, and Axum server.

## Main Types

- `ValidCode` is a validated short-link code. It accepts 1-32 ASCII letters, numbers, dashes, or underscores.
- `api::LinkRecord` is the persisted and returned representation of a short link.
- `api::DashboardData` is the authenticated dashboard payload.
- `api::CreateRequest` is the create-link request body.
- `api::LoginRequest` is the administrator login request body.
- `api::ApiError` is the JSON error payload returned by HTTP endpoints.

## Boundaries

This crate does not know about Axum, Leptos, sled, authentication, or HTTP status codes. It only contains reusable data contracts and invariants that should mean the same thing in every layer.

