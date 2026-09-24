#!/usr/bin/env bash
set -euo pipefail

ANVIL_PID_FILE="${ANVIL_PID_FILE:-/tmp/chainwatch-anvil.pid}"
if [[ -f "$ANVIL_PID_FILE" ]]; then
  pid="$(cat "$ANVIL_PID_FILE")"
  if kill -0 "$pid" 2>/dev/null; then
    kill "$pid" || true
    echo "Stopped Anvil pid $pid"
  fi
  rm -f "$ANVIL_PID_FILE"
else
  echo "No local Anvil pid file at $ANVIL_PID_FILE"
fi
