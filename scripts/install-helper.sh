#!/usr/bin/env bash
# Install braid-helper as a macOS LaunchDaemon (requires sudo).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LABEL="com.chaxus.braid.helper"
HELPER_DST="/Library/PrivilegedHelperTools/${LABEL}"
PLIST_SRC="${ROOT}/packaging/macos/${LABEL}.plist"
PLIST_DST="/Library/LaunchDaemons/${LABEL}.plist"

if [[ "$(id -u)" -ne 0 ]]; then
  exec sudo "$0" "$@"
fi

if [[ ! -f "${ROOT}/target/release/braid-helper" ]]; then
  echo "Building release braid-helper…"
  (cd "$ROOT" && cargo build --release --bin braid-helper)
fi

install -d /Library/PrivilegedHelperTools
install -m 755 "${ROOT}/target/release/braid-helper" "$HELPER_DST"
install -m 644 "$PLIST_SRC" "$PLIST_DST"

launchctl bootout system/"$LABEL" 2>/dev/null || true
launchctl bootstrap system "$PLIST_DST"
launchctl enable system/"$LABEL" 2>/dev/null || true
launchctl kickstart -k system/"$LABEL"

echo "Installed ${LABEL}."
echo "Socket: /var/run/braid-helper.sock"
echo "Uninstall: ./scripts/uninstall-helper.sh"
