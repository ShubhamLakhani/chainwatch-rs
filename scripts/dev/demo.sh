#!/usr/bin/env bash
# Five-minute local demo: infrastructure should already be up (`make up`).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"
API="${API_URL:-http://127.0.0.1:8080}"

echo "==> Seeding Anvil with DemoToken transfers"
"$ROOT/scripts/dev/seed.sh"

echo "==> Waiting for indexer API at $API"
for _ in {1..40}; do
  if curl -sf "$API/health" >/dev/null; then
    echo "API is up"
    break
  fi
  sleep 0.5
done

if ! curl -sf "$API/health" >/dev/null; then
  echo "Indexer API is not reachable at $API"
  echo "Start it in another terminal with: make run"
  echo "Then re-query:"
  echo "  curl -s $API/events | jq"
  exit 0
fi

echo "==> Waiting for events to be indexed"
for _ in {1..20}; do
  COUNT="$(curl -sf "$API/events" | python3 -c 'import json,sys; print(len(json.load(sys.stdin).get("events",[])))' 2>/dev/null || echo 0)"
  if [[ "$COUNT" != "0" ]]; then
    break
  fi
  sleep 1
done

echo
echo "==> /status"
curl -s "$API/status"
echo
echo
echo "==> /events"
curl -s "$API/events"
echo
echo
echo "Demo complete."
