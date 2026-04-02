#!/usr/bin/env bash
set -euo pipefail

app_pid=""

cleanup() {
  trap - EXIT INT TERM
  [[ -n "${app_pid:-}" ]] && kill "$app_pid" 2>/dev/null || true
  wait 2>/dev/null || true
}
trap cleanup EXIT INT TERM

# Kill anything on port 1420 from a previous run
stale_pids="$(lsof -t -nP -iTCP:1420 -sTCP:LISTEN 2>/dev/null || true)"
if [[ -n "$stale_pids" ]]; then
  kill $stale_pids 2>/dev/null || true
  sleep 1
fi

# Full Tauri desktop app (Vite is started by tauri's beforeDevCommand)
bun run dev &
app_pid=$!
wait "$app_pid"
