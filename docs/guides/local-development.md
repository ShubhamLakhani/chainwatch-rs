# Local development

Docker Desktop is optional. The default macOS path is local PostgreSQL plus
Foundry Anvil.

## Prerequisites

See the README. In particular on macOS, Anvil needs `libusb`
(`brew install libusb`).

## First run

```bash
cp .env.example .env
# Default DATABASE_URL=postgres://chainwatch:chainwatch@127.0.0.1:5432/chainwatch
# (Homebrew / Postgres.app: replace with your local username if not using Docker)
createdb chainwatch   # if needed
make up
make run              # other terminal
make seed
```

`make up` uses `docker compose -f compose.yaml` when Compose is available,
otherwise `scripts/dev/up.sh` (local Anvil, existing Postgres).

## Foundry project

Contracts live under `contracts/`:

```text
contracts/
  foundry.toml
  src/DemoToken.sol
  script/DeployDemoToken.s.sol
  test/DemoToken.t.sol
```

Commands:

```bash
cd contracts && forge test
# or
make contracts-test
```

`scripts/dev/seed.sh` runs `forge create` from that directory. Generated
`out/`, `cache/`, and `broadcast/` stay inside `contracts/` and are gitignored.

## Tests

```bash
cargo test --all-features           # Pure unit tests (no external services needed)
make test                           # Alias for cargo test --all-features
make test-integration               # Explicitly runs PostgreSQL and Anvil integration tests
make lint                           # Clippy checks
make fmt                            # Checks code formatting without modifying files (cargo fmt --all -- --check)
```

Developers can manually run `cargo fmt --all` when they intentionally want to reformat code.

Unit tests run instantly in isolation with no database or node dependencies.
Integration tests require a running PostgreSQL instance and Anvil node, and
are explicitly marked with `#[ignore]`. Running `make test-integration` loads
`.env` and runs these integration suites with full assertions. Missing infrastructure
during an integration run causes an explicit test failure rather than a silent skip.
Each integration test uses unique `chain_id` values so they do not collide
with the live indexer on chain `31337`.

## Two indexers

A second `make run` against the same database and `CHAIN_ID` fails with
`another indexer already holds the advisory lock`.

## Shutdown

Ctrl+C cancels the shared `CancellationToken`. Axum drains; the indexer stops
starting new batches.
