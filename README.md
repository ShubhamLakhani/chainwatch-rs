# chainwatch-rs

A production-style **EVM blockchain event indexer** written in Rust.

ChainWatch connects to an Ethereum-compatible JSON-RPC node, monitors ERC-20 `Transfer` events, processes them with bounded asynchronous concurrency, persists normalized events and checkpoints transactionally in PostgreSQL, and serves indexed data through a read-only Axum HTTP API.

The project demonstrates systems-level backend engineering: safe handling of 256-bit numbers, bounded concurrency against RPC endpoints, crash recovery with transactional checkpoints, and automated detection and rollback of chain reorganizations. It runs locally using **Foundry Anvil** and **PostgreSQL** without external dependencies, paid APIs, or real funds.

> **Engineering Positioning Note:** This repository demonstrates production-style systems architecture, concurrency control, and data reliability patterns. It is designed and maintained as a reproducible technical artifact rather than an active commercial deployment.

---

## Technical Problem Solved

EVM nodes are optimized for consensus, state transition execution, and cryptographic validation. They are poorly suited for relational queries such as:
- *"List all token transfers sent or received by address `0x...` in chronological order."*
- *"Fetch indexed transfers filtered by contract with pagination and instant API response."*

ChainWatch bridges this gap by continuously indexing blocks into PostgreSQL while enforcing strict correctness invariants:

```text
EVM Node (JSON-RPC) ──► RPC Fetcher (Semaphore Backpressure) ──► ERC-20 Decoder (U256)
                                                                        │
HTTP API & Metrics  ◄──  PostgreSQL (Tx Checkpoints & Lock)  ◄──────────┘
```

---

## Core Engineering Properties

- **Ethereum-Compatible JSON-RPC Ingestion:** Built over `reqwest` and `rustls`, supporting standard EVM RPC methods (`eth_blockNumber`, `eth_getBlockByNumber`, `eth_getLogs`, `eth_chainId`).
- **ERC-20 Transfer Decoding:** Decodes standard 3-topic Transfer events (`0xddf252...`), while safely filtering out 4-topic non-compliant logs (e.g., ERC-721 transfers) and malformed data.
- **U256-Safe Amount Handling:** Token balances and amounts use `alloy-primitives::U256` throughout the pipeline, completely avoiding floating-point precision loss and storing exact decimal string representations in PostgreSQL.
- **Bounded Tokio Concurrency & Backpressure:** Batch block and log fetching runs concurrently using Tokio `JoinSet`, strictly regulated by an asynchronous `Semaphore` (`MAX_CONCURRENT_REQUESTS`) to avoid node saturation.
- **Ordered Persistence:** Although block headers are retrieved concurrently, persistence is strictly sequential in block order. A block's events, block hash, and checkpoint commit together in a single PostgreSQL transaction.
- **PostgreSQL Transactional Checkpoints:** The indexer records its progress in `indexer_checkpoints`. On startup or crash recovery, it resumes from the latest checkpoint without gaps.
- **Idempotent Event Writes:** `indexed_events` enforces a unique constraint on `(chain_id, transaction_hash, log_index)` with `ON CONFLICT DO NOTHING`, guaranteeing that replay or retry never introduces duplicate records.
- **Crash & Restart Recovery:** Process restart safely resumes from the stored checkpoint. Replaying previously observed blocks does not duplicate rows or corrupt state.
- **Chain Reorganization Detection & Rollback:** On every sync tick, the indexer verifies that the stored checkpoint block hash matches the canonical chain. On mismatch, it walks backward to locate the common ancestor, rolls back orphaned events in a single database transaction, and resumes canonical indexing.
- **PostgreSQL Advisory Locking:** Acquires a session-level advisory lock on `CHAIN_ID` (`pg_try_advisory_lock`), preventing split-brain execution when multiple indexer instances target the same database.
- **Axum HTTP API:** Exposes clean, read-only endpoints: `/health`, `/ready`, `/status`, `/events`, and `/metrics`.
- **Telemetry & Metrics:** In-memory operational metrics track indexing lag, catch-up lag, total events, RPC retries, and reorganization count, available in JSON or Prometheus text format.
- **Automated Testing Suite:** 34 isolated unit tests (running with zero external dependencies) plus 5 explicit integration tests verifying PostgreSQL migrations, persistence idempotency, API routing, and Anvil RPC interaction.
- **Local Anvil Environment:** Includes a self-contained Foundry project (`contracts/`) with an ERC-20 token (`DemoToken.sol`), deployment scripts, and automated transfer seeding.
- **CI & Security Auditing:** Automated GitHub Actions workflows enforce `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `forge test`, and dependency security vulnerability checks via `cargo-deny`.

---

## Architecture Overview

```mermaid
flowchart TB
    subgraph bootstrap [bootstrap]
        Main["main.rs → run()"]
        Cfg[Config]
        Shut[CancellationToken]
    end

    subgraph interfaces [interfaces / HTTP]
        API["Axum /health /ready /status /events /metrics"]
    end

    subgraph application [application]
        Idx[Indexer loop]
        Pipe[Processor JoinSet + Semaphore]
        Reorg[Reorg detector]
    end

    subgraph infrastructure [infrastructure]
        RPC[HttpRpcClient]
        Dec[ERC-20 decoder]
        Repo[PostgreSQL repository]
        Tel[tracing + metrics]
    end

    subgraph domain [domain]
        Types["BlockHeader, DecodedTransfer, Checkpoint"]
    end

    Anvil[Anvil JSON-RPC] --> RPC
    Main --> Cfg
    Main --> Idx
    Main --> API
    Idx --> Pipe
    Idx --> Reorg
    Pipe --> RPC
    Pipe --> Dec
    Idx --> Repo
    Reorg --> Repo
    API --> Repo
    Repo --> PG[(PostgreSQL)]
    Idx --> Tel
    API --> Tel
    Dec --> Types
    Repo --> Types
```

Detailed architectural specifications:
- [Architecture Overview](docs/architecture/overview.md)
- [Indexing Pipeline & Concurrency](docs/architecture/indexing-pipeline.md)
- [Persistence & Transaction Semantics](docs/architecture/persistence.md)
- [Reorganization Handling](docs/architecture/reorg-handling.md)
- [ADR 0001: Single-Crate Layered Architecture](docs/adr/0001-single-crate-layered-architecture.md)

---

## Tech Stack

| Component | Technology | Rationale |
| --- | --- | --- |
| **Language** | Rust 2024 (MSRV 1.85) | Memory safety, zero-cost abstractions, predictable execution without GC |
| **Async Runtime** | Tokio 1 | Work-stealing scheduler, bounded concurrency channels, async timeouts |
| **HTTP API** | Axum 0.8 / Tower | High-throughput asynchronous HTTP routing integrated with Tokio |
| **Database** | PostgreSQL + SQLx 0.8 | ACID transaction boundaries, advisory locks, runtime queries with rustls |
| **RPC Client** | Reqwest 0.12 + Rustls | Fast HTTP client with exponential backoff retry and timeout handling |
| **EVM Primitives** | Alloy Primitives 1.0 | Standard cryptographic types (`Address`, `B256`, `U256`, Keccak-256) |
| **Logging & Tracing** | Tracing / Tracing-Subscriber | Structured, filterable diagnostic logging |
| **Local EVM** | Foundry Anvil | Instant local EVM execution with snapshot/revert support for reorg testing |

---

## Local Prerequisites

- **Xcode Command Line Tools** (on macOS)
- **Rust Toolchain:** Stable Rust (`rustup default stable`)
- **PostgreSQL 15+:** Running locally on port `5432` (Docker, Homebrew, or Postgres.app)
- **Foundry:** Provides `anvil`, `forge`, and `cast`:
  ```bash
  curl -L https://foundry.paradigm.xyz | bash && foundryup
  ```
- **cargo-deny:** Required for dependency auditing and security checks (`make check` / `make security`):
  ```bash
  cargo install cargo-deny --locked
  ```
- **Python 3:** Required by `scripts/dev/demo.sh` and `scripts/dev/reorg_demo.sh` for resilient JSON parsing without extra dependencies.
- **Docker & Compose:** Optional. Local scripts run natively on the host if Docker is not used.

---

## Quick Start (Local Reproduction)

### 1. Configure Environment
```bash
cp .env.example .env
```
Default `DATABASE_URL` targets Docker Compose (`postgres://chainwatch:chainwatch@127.0.0.1:5432/chainwatch`).
If using Homebrew or Postgres.app on macOS, update `.env` with your local user credentials:
```text
DATABASE_URL=postgres://<your-username>@127.0.0.1:5432/chainwatch
```

Create the database if it does not yet exist:
```bash
createdb chainwatch
```

### 2. Start Services
```bash
make up
```
Starts Anvil on port `8545` (and starts PostgreSQL via Docker if available).

### 3. Run the Indexer
In a second terminal:
```bash
make run
```
Applies database migrations, acquires the PostgreSQL advisory lock, and begins listening on `127.0.0.1:8080`.

### 4. Deploy Contract & Seed Events
In your first terminal:
```bash
make seed
```
Deploys `DemoToken.sol` using Anvil's local test account and generates multiple ERC-20 token transfers.

### 5. Query the API
```bash
curl -s http://127.0.0.1:8080/health | jq
curl -s http://127.0.0.1:8080/status | jq
curl -s http://127.0.0.1:8080/events | jq
```

---

## Live Reorganization Demonstration

ChainWatch includes an automated, live demonstration of chain reorganization handling:

```bash
make demo-reorg
# or
./scripts/dev/reorg_demo.sh
```

**Workflow:**
1. Connects to the local Anvil node (verifying chain ID 31337) and ChainWatch API.
2. Creates an EVM snapshot (`evm_snapshot`).
3. Broadcasts a transaction on **Branch A**; verifies ChainWatch indexes it and updates the checkpoint.
4. Reverts Anvil to the snapshot (`evm_revert`), resetting the chain tip.
5. Broadcasts an alternate transaction on **Branch B**, creating an alternate history with a conflicting block hash at the same height.
6. ChainWatch detects that the stored checkpoint hash diverged from the canonical chain.
7. The indexer walks to the common ancestor, issues a rollback transaction deleting orphaned Branch A rows, advances the checkpoint to Branch B, and increments `reorg_count`.
8. Verifies: Branch A is deleted, Branch B is indexed, and `reorg_count` incremented.

---

## Testing Strategy

The test suite separates pure unit tests from integration tests requiring active infrastructure:

```bash
# 1. Pure unit tests (Runs instantly, requires NO database or Anvil node)
make test
# or: cargo test --all-features

# 2. Integration tests (Requires running PostgreSQL & Anvil; fails explicitly if unavailable)
make test-integration

# 3. Static analysis & linting
make lint          # cargo clippy --all-targets --all-features -- -D warnings
make fmt           # cargo fmt --all -- --check

# 4. Dependency security audit
make security      # cargo deny check

# 5. Smart contract tests
make contracts-test # forge test in contracts/

# 6. Comprehensive verification
make check         # Runs fmt, check, clippy, unit tests, security, and contract tests
```

### Test Classification

| Test Suite | File | Type | Requirement |
| --- | --- | --- | --- |
| **Decoder Unit Tests** | `tests/decoder.rs` | Unit | None (Pure logic) |
| **Pipeline & Math Tests** | `tests/indexing.rs` (unit tests) | Unit | None (Pure logic) |
| **Reorg Algorithm Tests** | `tests/reorg.rs` (unit tests) | Unit | None (In-memory mock RPC) |
| **Domain & Config Tests** | `src/**` | Unit | None (Pure logic) |
| **HTTP API Integration** | `tests/api.rs` | Integration (`#[ignore]`) | PostgreSQL (`DATABASE_URL`) |
| **Persistence & Idempotency** | `tests/persistence.rs` | Integration (`#[ignore]`) | PostgreSQL (`DATABASE_URL`) |
| **Crash Replay Recovery** | `tests/persistence.rs` | Integration (`#[ignore]`) | PostgreSQL (`DATABASE_URL`) |
| **Reorg DB Rollback** | `tests/reorg.rs` | Integration (`#[ignore]`) | PostgreSQL (`DATABASE_URL`) |
| **RPC Client Connectivity** | `tests/indexing.rs` | Integration (`#[ignore]`) | Anvil RPC (`RPC_URL`) |

---

## HTTP API Reference

### Health & Readiness
- `GET /health` — Returns `200 OK` with `{"status":"ok"}`.
- `GET /ready` — Checks PostgreSQL and RPC connections. Returns `200 OK` when ready or `503 Service Unavailable` if either dependency is unreachable.

### Status & Telemetry
- `GET /status` — Current indexing status, latest block, checkpoint, confirmation depth, catch-up lag, and reorganization counter:
```json
{
  "chain_id": 31337,
  "confirmation_depth": 0,
  "last_indexed_block": 285,
  "last_indexed_block_hash": "0x...",
  "latest_observed_block": 285,
  "safe_head": 285,
  "indexing_lag": 0,
  "catch_up_lag": 0,
  "total_indexed_events": 12,
  "rpc_request_failures": 0,
  "retry_count": 0,
  "reorg_count": 1,
  "contract_address": null
}
```
- `GET /metrics` — In-process atomic telemetry counters. Supports both JSON and Prometheus format (`Accept: text/plain`).

### Events Querying
- `GET /events?limit=50&offset=0` — List indexed events with pagination.
- `GET /events?address=0x...` — Filter transfers where address is sender or recipient.
- `GET /events?contract=0x...` — Filter transfers by token contract address.
- `GET /events/:tx_hash` — Retrieve all decoded transfer events for a transaction hash.

*All transfer `amount` fields are returned as decimal strings to preserve 256-bit unsigned integer precision.*

---

## Makefile Target Reference

| Target | Description |
| --- | --- |
| `make setup` | Creates `.env` from `.env.example` if not already present |
| `make up` | Starts Anvil (and PostgreSQL if Docker is available) |
| `make down` | Stops Anvil and Docker services |
| `make run` | Starts the ChainWatch indexer and API (`cargo run`) |
| `make seed` | Deploys DemoToken and broadcasts sample transfer transactions |
| `make demo` | Executes full local walkthrough (seed and API queries) |
| `make demo-reorg` | Runs the automated live chain reorganization demonstration |
| `make test` | Runs pure unit tests (`cargo test --all-features`) |
| `make test-integration` | Loads `.env` and executes all ignored integration test suites |
| `make lint` | Runs Clippy with compiler warnings treated as errors |
| `make fmt` | Checks code formatting without modifying files (`cargo fmt --all -- --check`) |
| `make check` | Runs full static and test verification pipeline |
| `make security` | Runs `cargo deny check` for advisories, licenses, and banned crates |
| `make contracts-test` | Runs Foundry smart contract tests (`forge test`) |

---

## Architectural Tradeoffs & Current Limitations

- **HTTP JSON-RPC Polling:** Polls `eth_blockNumber` at a configurable interval (`POLL_INTERVAL_MS`). A high-volume production setup could augment this with `eth_subscribe` over WebSocket for lower tip-following latency.
- **Offset-Based Pagination:** The `/events` endpoint uses `limit`/`offset` pagination. For massive event volumes, cursor-based pagination (e.g. `(block_number, log_index)`) would offer superior performance at scale.
- **In-Memory Metrics:** Metrics are maintained via atomic counters in process memory. In a distributed deployment, metrics would be exported to Prometheus / Grafana.
- **Single-Process Advisory Lock:** Concurrency protection relies on PostgreSQL advisory locking on `CHAIN_ID`. Multi-region active-active deployments would require partition leases or leader election.

---

## License

MIT License. See [LICENSE](LICENSE) for details.
