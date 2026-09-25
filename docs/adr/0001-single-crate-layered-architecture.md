# ADR 0001: Single-crate layered architecture

## Status

Accepted.

## Context

`chainwatch-rs` is a working indexer that started as a flat `src/*.rs` module
list. It needed clearer modular boundaries, test isolation, and maintainability
without becoming a multi-crate Cargo workspace.

## Decision

Keep **one Cargo package** with a thin binary and a library. Organize the
library into:

- `domain` — types and errors, no Axum/SQLx
- `application` — indexing workflows and small ports (`ChainReader`, `EventStore`)
- `infrastructure` — RPC, decoder, Postgres, telemetry
- `interfaces` — HTTP
- `bootstrap` — wiring and shutdown

## Why not a workspace

A workspace would add lockfile/CI complexity and force versioning of internal
crates that are only consumed by this binary. There is no second deployable
artifact and no need to publish libraries. The layer split is enough.

## Consequences

- Tests import `chainwatch_rs::application::...` rather than flat modules.
- Ports exist only where mocking or HTTP/indexer sharing is real.
- Domain `AppError` maps infrastructure driver errors as strings so domain
  does not import SQLx or reqwest.
- Foundry lives in `contracts/` as its own project root, still not a Rust
  workspace member.
