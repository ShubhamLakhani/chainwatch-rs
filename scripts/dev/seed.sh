#!/usr/bin/env bash
# Deploy DemoToken to local Anvil and generate Transfer events.
#
# Anvil account #0 (PUBLIC, never use outside local development):
#   address:     0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
#   private key: 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

RPC_URL="${RPC_URL:-http://127.0.0.1:8545}"
PRIVATE_KEY="${PRIVATE_KEY:-0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80}"
ALICE="0x70997970C51812dc3A010C7d01b50e0d17dc79C8"
BOB="0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"
ALICE_KEY="0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"

if ! command -v forge >/dev/null 2>&1 || ! command -v cast >/dev/null 2>&1; then
  echo "forge/cast not found. Install Foundry and re-run:"
  echo "  curl -L https://foundry.paradigm.xyz | bash && foundryup"
  exit 1
fi

validate_local_rpc_url() {
  local url="$1"
  local pattern="^https?://(localhost|127\.0\.0\.1|\[::1\])(:[0-9]+)?(/.*)?$"
  [[ "$url" =~ $pattern ]]
}

# Guardrail: Never allow well-known Anvil development keys to be broadcast to a remote/public RPC
if ! validate_local_rpc_url "$RPC_URL"; then
  echo "ERROR: Refusing to broadcast transactions with well-known Anvil development keys to non-local RPC: $RPC_URL" >&2
  echo "RPC_URL must strictly target localhost, 127.0.0.1, or [::1]." >&2
  exit 1
fi

echo "Waiting for RPC at $RPC_URL ..."
RPC_READY=false
for _ in {1..40}; do
  if cast chain-id --rpc-url "$RPC_URL" >/dev/null 2>&1; then
    RPC_READY=true
    break
  fi
  sleep 0.25
done

if [ "$RPC_READY" != "true" ]; then
  echo "ERROR: Could not reach RPC endpoint at $RPC_URL" >&2
  exit 1
fi

CHAIN_ID="$(cast chain-id --rpc-url "$RPC_URL")"
if [[ "$CHAIN_ID" != "31337" ]]; then
  echo "ERROR: Connected RPC chain ID is '$CHAIN_ID', expected 31337 (Anvil local development chain)." >&2
  echo "Refusing to broadcast transactions with well-known Anvil development keys on non-Anvil chain ID." >&2
  exit 1
fi
echo "Connected to local Anvil chain id $CHAIN_ID (safety checks passed)"

INITIAL_SUPPLY="1000000000000000000000000" # 1,000,000 WATCH
echo "Deploying DemoToken from contracts/ ..."
DEPLOY_OUT="$(
  ( cd "$ROOT/contracts" && forge create src/DemoToken.sol:DemoToken \
    --rpc-url "$RPC_URL" \
    --private-key "$PRIVATE_KEY" \
    --broadcast \
    --constructor-args "$INITIAL_SUPPLY" )
)"
echo "$DEPLOY_OUT"

CONTRACT_ADDRESS="$(echo "$DEPLOY_OUT" | awk '/Deployed to:/ {print $3}')"
if [[ -z "${CONTRACT_ADDRESS:-}" ]]; then
  echo "Could not parse deployed contract address"
  exit 1
fi
echo "DemoToken deployed at $CONTRACT_ADDRESS"

transfer() {
  local from_key="$1"
  local to="$2"
  local amount="$3"
  cast send "$CONTRACT_ADDRESS" "transfer(address,uint256)" "$to" "$amount" \
    --rpc-url "$RPC_URL" --private-key "$from_key" >/dev/null
}

echo "Generating transfers..."
transfer "$PRIVATE_KEY" "$ALICE" "1000000000000000000000"
transfer "$PRIVATE_KEY" "$BOB" "2500000000000000000000"
transfer "$PRIVATE_KEY" "$ALICE" "500000000000000000000"
transfer "$ALICE_KEY" "$BOB" "250000000000000000000"
cast send "$CONTRACT_ADDRESS" "mint(address,uint256)" "$ALICE" "100000000000000000000" \
  --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY" >/dev/null

echo
echo "Contract: $CONTRACT_ADDRESS"
echo "Useful API queries (indexer must be running on :8080):"
echo "  curl -s http://127.0.0.1:8080/health | jq"
echo "  curl -s http://127.0.0.1:8080/status | jq"
echo "  curl -s http://127.0.0.1:8080/events | jq"
echo "  curl -s 'http://127.0.0.1:8080/events?contract=${CONTRACT_ADDRESS}' | jq"
echo "  curl -s 'http://127.0.0.1:8080/events?address=${ALICE}' | jq"
echo
echo "$CONTRACT_ADDRESS" > /tmp/chainwatch-demo-token.txt
echo "Saved address to /tmp/chainwatch-demo-token.txt"
