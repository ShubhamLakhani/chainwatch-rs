#!/usr/bin/env bash
# Live local blockchain reorganization (reorg) demonstration for ChainWatch.
# Operates safely ONLY against local Anvil (chain ID 31337).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

RPC_URL="${RPC_URL:-http://127.0.0.1:8545}"
API_URL="${API_URL:-http://127.0.0.1:8080}"
PRIVATE_KEY="${PRIVATE_KEY:-0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80}"
ALICE="0x70997970C51812dc3A010C7d01b50e0d17dc79C8"
BOB="0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"

echo "================================================================"
echo "          ChainWatch Live Reorg Demonstration                   "
echo "================================================================"

validate_local_rpc_url() {
  local url="$1"
  local pattern="^https?://(localhost|127\.0\.0\.1|\[::1\])(:[0-9]+)?(/.*)?$"
  [[ "$url" =~ $pattern ]]
}

# ----------------------------------------------------------------------
# 1. Safety Checks: Only allow local Anvil chain ID 31337
# ----------------------------------------------------------------------
if ! validate_local_rpc_url "$RPC_URL"; then
  echo "ERROR: Refusing to run reorg demo against non-local RPC: $RPC_URL" >&2
  echo "This demo modifies chain state via Anvil RPC and must strictly target localhost, 127.0.0.1, or [::1]." >&2
  exit 1
fi

if ! command -v cast >/dev/null 2>&1; then
  echo "ERROR: 'cast' not found. Please install Foundry: curl -L https://foundry.paradigm.xyz | bash && foundryup" >&2
  exit 1
fi

echo "[1/7] Verifying Anvil RPC connectivity..."
if ! cast chain-id --rpc-url "$RPC_URL" >/dev/null 2>&1; then
  echo "ERROR: Cannot connect to Anvil RPC at $RPC_URL." >&2
  echo "Start Anvil with: make up" >&2
  exit 1
fi

CHAIN_ID="$(cast chain-id --rpc-url "$RPC_URL")"
if [[ "$CHAIN_ID" != "31337" ]]; then
  echo "ERROR: Connected RPC chain ID is '$CHAIN_ID', expected 31337 (Anvil local development chain)." >&2
  echo "Refusing to execute EVM snapshot/revert on non-Anvil network." >&2
  exit 1
fi
echo "  -> Connected to Anvil (chain id $CHAIN_ID, safety checks passed)"

echo "[2/7] Verifying ChainWatch API availability..."
if ! curl -sf "$API_URL/health" >/dev/null 2>&1; then
  echo "ERROR: ChainWatch API is not running at $API_URL." >&2
  echo "Start the indexer in another terminal with: make run" >&2
  exit 1
fi
echo "  -> ChainWatch API is healthy at $API_URL"

# Helper for JSON parsing using python3
json_val() {
  python3 -c "import json,sys; obj=json.load(sys.stdin); print(obj$1)" 2>/dev/null || echo ""
}

# ----------------------------------------------------------------------
# 2. Ensure DemoToken is deployed
# ----------------------------------------------------------------------
echo "[3/7] Ensuring DemoToken contract is available..."
CONTRACT_ADDRESS=""
TOKEN_FILE="/tmp/chainwatch-demo-token.txt"
if [[ -f "$TOKEN_FILE" ]]; then
  CANDIDATE="$(cat "$TOKEN_FILE" | tr -d '[:space:]')"
  if [[ -n "$CANDIDATE" ]]; then
    CODE="$(cast code "$CANDIDATE" --rpc-url "$RPC_URL" 2>/dev/null || echo "")"
    if [[ "$CODE" != "0x" && -n "$CODE" ]]; then
      CONTRACT_ADDRESS="$CANDIDATE"
    fi
  fi
fi

if [[ -z "$CONTRACT_ADDRESS" ]]; then
  echo "  -> Deploying new DemoToken to Anvil..."
  INITIAL_SUPPLY="1000000000000000000000000"
  DEPLOY_OUT="$(
    ( cd "$ROOT/contracts" && forge create src/DemoToken.sol:DemoToken \
      --rpc-url "$RPC_URL" \
      --private-key "$PRIVATE_KEY" \
      --broadcast \
      --constructor-args "$INITIAL_SUPPLY" )
  )"
  CONTRACT_ADDRESS="$(echo "$DEPLOY_OUT" | awk '/Deployed to:/ {print $3}')"
  if [[ -z "${CONTRACT_ADDRESS:-}" ]]; then
    echo "ERROR: Could not parse deployed contract address" >&2
    exit 1
  fi
  echo "$CONTRACT_ADDRESS" > "$TOKEN_FILE"
fi
echo "  -> Using DemoToken at $CONTRACT_ADDRESS"

# Query current confirmation depth and reorg count from API
STATUS_JSON="$(curl -sf "$API_URL/status")"
CONF_DEPTH="$(echo "$STATUS_JSON" | json_val '.get("confirmation_depth", 0)')"
INITIAL_REORG_COUNT="$(echo "$STATUS_JSON" | json_val '.get("reorg_count", 0)')"
echo "  -> Initial reorg_count: $INITIAL_REORG_COUNT"
echo "  -> Indexer confirmation_depth: $CONF_DEPTH"

# Function to mine blocks if confirmation_depth > 0
advance_confirmations() {
  if [[ "$CONF_DEPTH" -gt 0 ]]; then
    cast rpc anvil_mine "$(( CONF_DEPTH + 1 ))" --rpc-url "$RPC_URL" >/dev/null 2>&1 || true
  fi
}

# Wait for indexer to catch up with baseline
advance_confirmations
for _ in {1..30}; do
  LAG="$(curl -sf "$API_URL/status" | json_val '.get("catch_up_lag", 0)')"
  if [[ "$LAG" == "0" ]]; then
    break
  fi
  sleep 0.5
done

# ----------------------------------------------------------------------
# 3. Take Anvil EVM Snapshot
# ----------------------------------------------------------------------
echo "[4/7] Taking Anvil state snapshot before divergence..."
SNAPSHOT_RAW="$(cast rpc evm_snapshot --rpc-url "$RPC_URL")"
SNAPSHOT_ID="$(echo "$SNAPSHOT_RAW" | tr -d '"[:space:]')"
COMMON_ANCESTOR_BLOCK="$(cast block-number --rpc-url "$RPC_URL")"
COMMON_ANCESTOR_HASH="$(cast block "$COMMON_ANCESTOR_BLOCK" --rpc-url "$RPC_URL" --json | json_val '["hash"]')"
echo "  -> Snapshot ID: $SNAPSHOT_ID"
echo "  -> Common ancestor: block #$COMMON_ANCESTOR_BLOCK (hash $COMMON_ANCESTOR_HASH)"

# ----------------------------------------------------------------------
# 4. Branch A: Broadcast transaction and wait for indexing
# ----------------------------------------------------------------------
echo "[5/7] Branch A: Broadcasting transfer on fork A..."
# Unique amount: 777 WATCH tokens
AMOUNT_A="777000000000000000000"
TX_A_JSON="$(cast send "$CONTRACT_ADDRESS" "transfer(address,uint256)" "$ALICE" "$AMOUNT_A" \
  --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY" --json)"
TX_A_HASH="$(echo "$TX_A_JSON" | json_val '["transactionHash"]')"
BLOCK_A_HEX="$(echo "$TX_A_JSON" | json_val '["blockNumber"]')"
BLOCK_A="$(printf "%d" "$BLOCK_A_HEX")"
HASH_A="$(cast block "$BLOCK_A" --rpc-url "$RPC_URL" --json | json_val '["hash"]')"

echo "  -> Branch A Transaction: $TX_A_HASH"
echo "  -> Branch A Block: #$BLOCK_A (hash $HASH_A)"

advance_confirmations

echo "  -> Waiting for ChainWatch to index Branch A..."
INDEXED_A=false
for _ in {1..40}; do
  EVENTS_JSON="$(curl -sf "$API_URL/events?limit=50")"
  HAS_A="$(echo "$EVENTS_JSON" | python3 -c "
import json, sys
data = json.load(sys.stdin)
txs = [e.get('transaction_hash', '').lower() for e in data.get('events', [])]
print('true' if '$TX_A_HASH'.lower() in txs else 'false')
")"
  if [[ "$HAS_A" == "true" ]]; then
    INDEXED_A=true
    break
  fi
  sleep 0.5
done

if [[ "$INDEXED_A" != "true" ]]; then
  echo "FAIL: Timed out waiting for ChainWatch to index Branch A event ($TX_A_HASH)." >&2
  exit 1
fi
echo "  -> Branch A indexed successfully by ChainWatch."

# ----------------------------------------------------------------------
# 5. Revert Anvil to Snapshot & Produce Branch B
# ----------------------------------------------------------------------
echo "[6/7] Simulating reorganization: reverting Anvil to snapshot $SNAPSHOT_ID..."
REVERT_STATUS="$(cast rpc evm_revert "$SNAPSHOT_ID" --rpc-url "$RPC_URL" | tr -d '"[:space:]')"
if [[ "$REVERT_STATUS" != "true" ]]; then
  echo "ERROR: Failed to revert Anvil to snapshot $SNAPSHOT_ID." >&2
  exit 1
fi

REVERTED_BLOCK="$(cast block-number --rpc-url "$RPC_URL")"
echo "  -> Anvil reverted to block #$REVERTED_BLOCK (matching ancestor block #$COMMON_ANCESTOR_BLOCK)"

echo "  -> Branch B: Broadcasting alternate transfer on fork B..."
# Unique amount: 888 WATCH tokens to Bob
AMOUNT_B="888000000000000000000"
TX_B_JSON="$(cast send "$CONTRACT_ADDRESS" "transfer(address,uint256)" "$BOB" "$AMOUNT_B" \
  --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY" --json)"
TX_B_HASH="$(echo "$TX_B_JSON" | json_val '["transactionHash"]')"
BLOCK_B_HEX="$(echo "$TX_B_JSON" | json_val '["blockNumber"]')"
BLOCK_B="$(printf "%d" "$BLOCK_B_HEX")"
HASH_B="$(cast block "$BLOCK_B" --rpc-url "$RPC_URL" --json | json_val '["hash"]')"

echo "  -> Branch B Transaction: $TX_B_HASH"
echo "  -> Branch B Block: #$BLOCK_B (hash $HASH_B)"

if [[ "$HASH_A" == "$HASH_B" ]]; then
  echo "ERROR: Branch A and Branch B produced identical block hashes; reorg cannot be demonstrated." >&2
  exit 1
fi

advance_confirmations

# ----------------------------------------------------------------------
# 6. Verify ChainWatch Detects Reorg, Rolls Back A, and Indexes B
# ----------------------------------------------------------------------
echo "[7/7] Waiting for ChainWatch to detect reorg, rollback Branch A, and index Branch B..."
REORG_DETECTED=false
BRANCH_A_ORPHANED=false
BRANCH_B_INDEXED=false
FINAL_REORG_COUNT="$INITIAL_REORG_COUNT"

for _ in {1..50}; do
  STATUS_JSON="$(curl -sf "$API_URL/status")"
  CURRENT_REORG="$(echo "$STATUS_JSON" | json_val '.get("reorg_count", 0)')"
  FINAL_REORG_COUNT="$CURRENT_REORG"

  EVENTS_JSON="$(curl -sf "$API_URL/events?limit=50")"
  CHECK_STATUS="$(echo "$EVENTS_JSON" | python3 -c "
import json, sys
data = json.load(sys.stdin)
txs = [e.get('transaction_hash', '').lower() for e in data.get('events', [])]
has_a = '$TX_A_HASH'.lower() in txs
has_b = '$TX_B_HASH'.lower() in txs
print(f'{\"true\" if has_a else \"false\"},{\"true\" if has_b else \"false\"}')
")"
  HAS_A="$(echo "$CHECK_STATUS" | cut -d',' -f1)"
  HAS_B="$(echo "$CHECK_STATUS" | cut -d',' -f2)"

  if [[ "$CURRENT_REORG" -gt "$INITIAL_REORG_COUNT" ]]; then
    REORG_DETECTED=true
  fi
  if [[ "$HAS_A" == "false" ]]; then
    BRANCH_A_ORPHANED=true
  fi
  if [[ "$HAS_B" == "true" ]]; then
    BRANCH_B_INDEXED=true
  fi

  if [[ "$REORG_DETECTED" == "true" && "$BRANCH_A_ORPHANED" == "true" && "$BRANCH_B_INDEXED" == "true" ]]; then
    break
  fi
  sleep 0.5
done

# ----------------------------------------------------------------------
# 7. Summary & Result
# ----------------------------------------------------------------------
echo
echo "================================================================"
echo "                  REORG DEMONSTRATION SUMMARY                   "
echo "================================================================"
printf "%-40s : %s\n" "Local RPC (chain 31337) safe check" "PASS"
printf "%-40s : %s\n" "Branch A indexed ($TX_A_HASH)" "$( [ "$INDEXED_A" = "true" ] && echo "PASS" || echo "FAIL" )"
printf "%-40s : %s\n" "Anvil snapshot reverted ($SNAPSHOT_ID)" "PASS"
printf "%-40s : %s\n" "Reorg detected by indexer" "$( [ "$REORG_DETECTED" = "true" ] && echo "PASS" || echo "FAIL" )"
printf "%-40s : %s\n" "Branch A orphaned data rolled back" "$( [ "$BRANCH_A_ORPHANED" = "true" ] && echo "PASS" || echo "FAIL" )"
printf "%-40s : %s\n" "Branch B canonical data indexed" "$( [ "$BRANCH_B_INDEXED" = "true" ] && echo "PASS" || echo "FAIL" )"
printf "%-40s : %s (from %s to %s)\n" "Reorg metric incremented" "$( [ "$FINAL_REORG_COUNT" -gt "$INITIAL_REORG_COUNT" ] && echo "PASS" || echo "FAIL" )" "$INITIAL_REORG_COUNT" "$FINAL_REORG_COUNT"
echo "----------------------------------------------------------------"

if [[ "$INDEXED_A" == "true" && "$REORG_DETECTED" == "true" && "$BRANCH_A_ORPHANED" == "true" && "$BRANCH_B_INDEXED" == "true" ]]; then
  echo "RESULT: PASS - All reorg detection and rollback invariants verified."
  exit 0
else
  echo "RESULT: FAIL - One or more reorg invariants failed."
  exit 1
fi
