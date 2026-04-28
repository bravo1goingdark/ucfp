#!/usr/bin/env bash
# Periodic snapshot of the redb file to Cloudflare R2.
#
# Designed to run as a sidecar / cron inside the same Container as the ucfp
# server. Cloudflare Containers' R2 binding mounts the bucket at
# /mnt/r2 (configurable). On restart, the entrypoint should restore from the
# most recent snapshot before starting the server.
#
# Usage (foreground):  scripts/redb-snapshot.sh loop
# Usage (one shot):    scripts/redb-snapshot.sh once
# Usage (restore):     scripts/redb-snapshot.sh restore
set -euo pipefail

DATA_DIR="${UCFP_DATA_DIR:-/data}"
SNAPSHOT_DIR="${UCFP_SNAPSHOT_DIR:-/mnt/r2/snapshots}"
SNAPSHOT_INTERVAL="${UCFP_SNAPSHOT_INTERVAL:-300}"  # seconds
KEEP_LAST="${UCFP_SNAPSHOT_KEEP:-24}"                # rolling N

mkdir -p "$SNAPSHOT_DIR"

snapshot_once() {
    local src="$DATA_DIR/ucfp.redb"
    [[ -f "$src" ]] || { echo "no redb file at $src yet, skipping"; return 0; }

    local stamp dst
    stamp=$(date -u +%Y%m%dT%H%M%SZ)
    dst="$SNAPSHOT_DIR/ucfp-$stamp.redb"

    # Atomic copy (redb tolerates being copied while open thanks to MVCC,
    # but cp + sync gives us a crash-consistent snapshot).
    cp -f "$src" "$dst.tmp"
    sync
    mv -f "$dst.tmp" "$dst"
    echo "snapshot: $dst ($(stat -c%s "$dst") bytes)"

    # Prune oldest beyond KEEP_LAST.
    ls -1t "$SNAPSHOT_DIR"/ucfp-*.redb 2>/dev/null | tail -n +"$((KEEP_LAST + 1))" | xargs -r rm -f
}

restore_latest() {
    local latest
    latest=$(ls -1t "$SNAPSHOT_DIR"/ucfp-*.redb 2>/dev/null | head -n 1 || true)
    if [[ -z "$latest" ]]; then
        echo "no snapshot to restore from"; return 0
    fi
    cp -f "$latest" "$DATA_DIR/ucfp.redb"
    echo "restored from $latest"
}

case "${1:-loop}" in
    once)    snapshot_once ;;
    restore) restore_latest ;;
    loop)
        echo "snapshot loop: every ${SNAPSHOT_INTERVAL}s, keep ${KEEP_LAST}"
        while true; do
            snapshot_once || echo "snapshot failed (continuing)"
            sleep "$SNAPSHOT_INTERVAL"
        done
        ;;
    *)
        echo "usage: $0 {loop|once|restore}" >&2
        exit 2
        ;;
esac
