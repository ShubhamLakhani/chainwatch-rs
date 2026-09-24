# Persistence

PostgreSQL is the source of truth for indexed events, block hashes, and the
resume checkpoint.

## Atomic block writes

`persist_block` opens one transaction and, inside it:

1. Inserts each event with `ON CONFLICT DO NOTHING`.
2. Upserts `indexed_blocks`.
3. Upserts `indexer_checkpoints`.
4. `COMMIT`.

If step 2 fails, step 1 is rolled back. The checkpoint never moves past events
that were not stored.

`rollback_after` is the inverse transaction: delete events and blocks above
the ancestor, then write the checkpoint back to that ancestor.

`reset_chain` deletes all rows for a `chain_id` (used when genesis itself
changed, typical after a new Anvil process).

## Idempotency: at-least-once vs effectively-once

JSON-RPC `eth_getLogs` can return the same log twice. The indexer is
at-least-once on the network.

The unique constraint `(chain_id, transaction_hash, log_index)` plus
`ON CONFLICT DO NOTHING` makes persistence effectively-once. Replaying a
block after a crash inserts zero extra rows and still refreshes the
checkpoint.

Token amounts are stored as decimal strings from `U256`, never as floats.

## Schema

Applied in `migrations/0001_initial.sql`. Do not rewrite that file after it
has been applied; add a new migration if the schema must change.

Useful indexes already exist on address/contract/block columns. SQL is
parameterized. The pool is capped at 10 connections.

## Mapping

SQLx `FromRow` structs live in `infrastructure/persistence/rows.rs`. Domain
`IndexedEvent` is produced at that boundary. HTTP `EventDto` is produced in
`interfaces/http/dto.rs`. Handlers do not return raw SQLx rows.

## Advisory lock

At startup the process checks out a dedicated connection and calls
`pg_try_advisory_lock` namespaced for this app and `CHAIN_ID`. The connection
is held until shutdown so the session lock is not returned to the pool. A
second indexer on the same chain fails fast instead of writing competing
checkpoints.
