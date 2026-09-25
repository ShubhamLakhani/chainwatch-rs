#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

cargo fmt --all -- --check
cargo check --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features

if ! command -v cargo-deny >/dev/null 2>&1; then
  echo "ERROR: 'cargo-deny' is required by make check but was not found." >&2
  echo "Install with: cargo install cargo-deny --locked" >&2
  exit 1
fi
cargo deny check

if ! command -v forge >/dev/null 2>&1; then
  echo "ERROR: 'forge' is required by make check but was not found." >&2
  echo "Install Foundry with: curl -L https://foundry.paradigm.xyz | bash && foundryup" >&2
  exit 1
fi
( cd "$ROOT/contracts" && forge test )
