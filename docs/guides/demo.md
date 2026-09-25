# Five-minute demo

Goal: show a working indexer on your laptop. No paid RPC. No real funds.

You need two terminals. Commands assume you are in the `chainwatch-rs`
directory.

## 0. One-time setup

```bash
cp .env.example .env
# Edit DATABASE_URL if your Postgres user is not the example user.
```

Install:

- Rust stable (`rustup`)
- PostgreSQL 15+ listening on `127.0.0.1:5432`
- Foundry so `anvil`, `forge`, and `cast` are on your `PATH`
- Python 3 (`python3`, used by `demo.sh` and `reorg_demo.sh` for JSON parsing)
- Optional: Docker Desktop

Create the database once if you are not using Compose:

```bash
createdb chainwatch
```

## 1. Start infrastructure (30 seconds)

```bash
make up
```

This starts Anvil on `:8545`. If Docker Compose is installed it also starts
Postgres. On this project’s default local path, Homebrew/Postgres.app can
already be running on `:5432`.

Confirm:

```bash
cast chain-id --rpc-url http://127.0.0.1:8545
# 31337
```

## 2. Start the indexer (terminal 2)

```bash
make run
```

You should see migrations applied, `connected to EVM JSON-RPC`, and
`HTTP API listening` on `127.0.0.1:8080`.

```bash
curl -s http://127.0.0.1:8080/health
# {"status":"ok"}
```

## 3. Deploy the demo token and generate transfers

In terminal 1:

```bash
make seed
```

The script prints the contract address and sample `curl` commands. It uses
Anvil’s well-known account #0. Those keys are public and are **not** for
production.

## 4. Query indexed events

Wait a couple of seconds (poll interval is 1s, confirmation depth is 0 in
`.env.example`).

```bash
curl -s http://127.0.0.1:8080/events | jq
curl -s http://127.0.0.1:8080/status | jq
curl -s http://127.0.0.1:8080/ready | jq
```

Filter:

```bash
ADDR=0x70997970C51812dc3A010C7d01b50e0d17dc79C8
curl -s "http://127.0.0.1:8080/events?address=$ADDR" | jq
```

## 5. Check synchronization status

```bash
curl -s http://127.0.0.1:8080/status | jq
curl -s http://127.0.0.1:8080/metrics | jq
```

Talking points:

- `last_indexed_block` should track Anvil’s tip.
- `indexing_lag` should be ~0 with `CONFIRMATION_DEPTH=0`.
- `total_indexed_events` should match the transfers you sent (including the
  constructor mint).

## 6. Inspect the database

```bash
psql "$DATABASE_URL" -c "SELECT last_indexed_block, last_indexed_block_hash FROM indexer_checkpoints;"
psql "$DATABASE_URL" -c "SELECT block_number, sender_address, recipient_address, amount FROM indexed_events ORDER BY id;"
```

## 7. Restart recovery

In the indexer terminal press Ctrl+C, then `make run` again. The log line
`resuming from persisted checkpoint` should appear. `/events` should return
the same rows — no duplicates. That is the unique constraint plus checkpoint
resume.

## 8. Live blockchain reorganization demonstration

To watch ChainWatch detect and roll back a chain reorganization live on Anvil:

```bash
make demo-reorg
# or
./scripts/dev/reorg_demo.sh
```

What this script does:
1. Verifies local Anvil (chain 31337) and ChainWatch API health.
2. Records the baseline block and takes an Anvil EVM snapshot (`evm_snapshot`).
3. Broadcasts a token transfer on **Branch A** (e.g. 777 tokens) and verifies ChainWatch indexes it.
4. Reverts Anvil to the earlier snapshot (`evm_revert`), resetting the chain tip.
5. Broadcasts an alternate transaction on **Branch B** (e.g. 888 tokens to Bob), producing an alternate canonical history with a conflicting block hash at the same block height.
6. The ChainWatch indexer detects that its stored checkpoint block hash diverged from the canonical chain.
7. The indexer walks back to the common ancestor, triggers a database transaction that deletes the orphaned Branch A rows, advances the checkpoint to Branch B, and increments `reorg_count`.
8. The demo asserts that Branch A is deleted, Branch B is indexed, and `reorg_count` increased.

## 9. Stop

```bash
# terminal 2: Ctrl+C
make down
```
