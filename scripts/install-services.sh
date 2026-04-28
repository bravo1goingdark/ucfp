#!/usr/bin/env bash
# Install ucfp systemd services for the current user.
# Run once after building the binary:
#   bash scripts/install-services.sh
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
USER="${SUDO_USER:-$USER}"

# Copy cloudflared to a stable location if not already there.
if [ ! -x /usr/local/bin/cloudflared ]; then
  if [ -x /tmp/cloudflared ]; then
    sudo cp /tmp/cloudflared /usr/local/bin/cloudflared
    echo "Installed cloudflared to /usr/local/bin/cloudflared"
  else
    echo "ERROR: cloudflared not found. Download it first:" >&2
    echo "  curl -fsSL https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64 -o /tmp/cloudflared && chmod +x /tmp/cloudflared" >&2
    exit 1
  fi
fi

chmod +x "$REPO_DIR/scripts/ucfp-server-start.sh" \
         "$REPO_DIR/scripts/ucfp-tunnel-start.sh"

# Create .env file with required secrets if it doesn't exist.
ENV_FILE="$REPO_DIR/.env.ucfp"
if [ ! -f "$ENV_FILE" ]; then
  cat > "$ENV_FILE" << ENV
# UCFP server environment — loaded by ucfp-server.service
UCFP_TOKEN=17ca09974a9d5c7f60be0091c32fa693dc0ed1d2e68b551fbe0120bbb1d2f9aa
UCFP_BIND=0.0.0.0:8080
UCFP_DATA_DIR=$REPO_DIR/data
ENV
  echo "Created $ENV_FILE"
fi

# Install user-level systemd units.
UNIT_DIR="$HOME/.config/systemd/user"
mkdir -p "$UNIT_DIR"

for svc in ucfp-server.service ucfp-tunnel.service; do
  # Substitute %i → actual username in the service files.
  sed "s|%i|$USER|g" "$REPO_DIR/scripts/$svc" > "$UNIT_DIR/$svc"
  echo "Installed $UNIT_DIR/$svc"
done

systemctl --user daemon-reload
systemctl --user enable ucfp-server.service ucfp-tunnel.service
systemctl --user start  ucfp-server.service ucfp-tunnel.service

echo ""
echo "Services installed and started. Check status with:"
echo "  systemctl --user status ucfp-server ucfp-tunnel"
echo "  journalctl --user -u ucfp-tunnel -f"
