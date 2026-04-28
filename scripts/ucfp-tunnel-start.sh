#!/usr/bin/env bash
# Start cloudflared quick tunnel, wait for URL, push it to CF Pages secret,
# then trigger a redeploy.  Designed to run as ExecStart for
# ucfp-tunnel.service — systemd restarts it on failure so the URL stays fresh.
set -euo pipefail

CLOUDFLARED=${CLOUDFLARED_BIN:-/usr/local/bin/cloudflared}
LOG=/tmp/cf_tunnel.log
BACKEND_PORT=${UCFP_BACKEND_PORT:-8080}
CF_PROJECT=${CF_PAGES_PROJECT:-ucfp}

cleanup() { rm -f "$LOG"; }
trap cleanup EXIT

# Start tunnel; log goes to file so we can grep the URL.
"$CLOUDFLARED" tunnel --url "http://localhost:$BACKEND_PORT" >"$LOG" 2>&1 &
CF_PID=$!

# Wait up to 30s for the trycloudflare URL.
TUNNEL_URL=""
for i in $(seq 1 30); do
  TUNNEL_URL=$(grep -o 'https://[^ ]*\.trycloudflare\.com' "$LOG" 2>/dev/null | head -1 || true)
  [ -n "$TUNNEL_URL" ] && break
  sleep 1
done

if [ -z "$TUNNEL_URL" ]; then
  echo "ERROR: cloudflared did not emit a trycloudflare URL in 30s" >&2
  kill $CF_PID 2>/dev/null || true
  exit 1
fi

echo "Tunnel URL: $TUNNEL_URL"

# Push the new URL as the Pages secret and trigger a redeploy.
if command -v wrangler >/dev/null 2>&1; then
  echo "$TUNNEL_URL" | wrangler pages secret put UCFP_API_URL \
    --project-name "$CF_PROJECT" >/dev/null 2>&1 && \
    echo "UCFP_API_URL updated in Cloudflare Pages."

  # Redeploy by re-uploading the last build artifact (if available).
  SVELTE_OUT="$(dirname "$(dirname "$0")")/web/.svelte-kit/cloudflare"
  if [ -d "$SVELTE_OUT" ]; then
    wrangler pages deploy "$SVELTE_OUT" --project-name "$CF_PROJECT" \
      --commit-dirty=true >/dev/null 2>&1 && \
      echo "Pages redeployed with new tunnel URL."
  fi
fi

# Block until cloudflared exits (systemd will restart us if it does).
wait $CF_PID
