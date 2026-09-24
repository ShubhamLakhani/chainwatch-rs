#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

ANVIL_LOG="${ANVIL_LOG:-/tmp/chainwatch-anvil.log}"
ANVIL_PID_FILE="${ANVIL_PID_FILE:-/tmp/chainwatch-anvil.pid}"

if curl -sf -X POST http://127.0.0.1:8545 \
  -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' >/dev/null; then
  echo "Anvil already responding on :8545"
else
  if ! command -v anvil >/dev/null 2>&1; then
    echo "anvil not found. Install Foundry: https://book.getfoundry.sh/getting-started/installation"
    echo "  curl -L https://foundry.paradigm.xyz | bash && foundryup"
    exit 1
  fi
  echo "Starting Anvil on :8545 (chain id 31337, 1s block time)"
  nohup anvil --host 127.0.0.1 --port 8545 --chain-id 31337 --block-time 1 >"$ANVIL_LOG" 2>&1 &
  echo $! >"$ANVIL_PID_FILE"
  for _ in {1..30}; do
    if curl -sf -X POST http://127.0.0.1:8545 \
      -H 'content-type: application/json' \
      --data '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' >/dev/null; then
      echo "Anvil is ready"
      break
    fi
    sleep 0.3
  done
fi

if ! command -v pg_isready >/dev/null 2>&1; then
  for pg_candidate in \
    /opt/homebrew/bin \
    /usr/local/bin \
    /opt/homebrew/opt/postgresql@*/bin \
    /usr/local/opt/postgresql@*/bin \
    /Applications/Postgres.app/Contents/Versions/latest/bin; do
    for dir in $pg_candidate; do
      if [ -x "$dir/pg_isready" ]; then
        export PATH="$dir:$PATH"
        break 2
      fi
    done
  done
fi

if command -v pg_isready >/dev/null 2>&1; then
  pg_isready -h 127.0.0.1 -p 5432 >/dev/null && echo "PostgreSQL is accepting connections on :5432"
else
  echo "Notice: pg_isready not found in PATH. Ensure PostgreSQL is running on :5432."
fi
