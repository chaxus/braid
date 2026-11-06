#!/usr/bin/env bash
# Generate the Braid Host + Packet Tunnel Xcode project and open it.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MACOS="$ROOT/macos"
cd "$MACOS"

if [[ -z "${DEVELOPMENT_TEAM:-}" ]]; then
  if [[ -f "$MACOS/Config/Local.xcconfig" ]]; then
    # shellcheck disable=SC1091
    DEVELOPMENT_TEAM="$(grep -E '^DEVELOPMENT_TEAM' "$MACOS/Config/Local.xcconfig" | awk -F= '{print $2}' | tr -d '[:space:]')"
    export DEVELOPMENT_TEAM
  fi
fi

if [[ -z "${DEVELOPMENT_TEAM:-}" || "${DEVELOPMENT_TEAM}" == "YOUR_TEAM_ID" ]]; then
  echo "Set DEVELOPMENT_TEAM to your Apple Team ID, e.g.:" >&2
  echo "  export DEVELOPMENT_TEAM=ABCD123456" >&2
  echo "  cp macos/Config/Local.xcconfig.example macos/Config/Local.xcconfig" >&2
  echo "Continuing project generation; signing will fail until Team is set." >&2
fi

if ! command -v xcodegen >/dev/null 2>&1; then
  echo "xcodegen not found. Install with: brew install xcodegen" >&2
  exit 1
fi

echo "Generating Braid.xcodeproj (DEVELOPMENT_TEAM=${DEVELOPMENT_TEAM:-unset})…"
xcodegen generate
open Braid.xcodeproj

cat <<'EOF'

Next in Xcode:
  1. Select the BraidHost target → Signing & Capabilities → your Team
  2. Confirm App Groups: group.com.chaxus.braid
  3. Confirm Network Extensions → Packet Tunnel
  4. Build & run BraidHost once so the appex is installed
  5. Use `BRAID_LIVE=1 nsh run` for the UI; Connect will prefer NE when available

App Group: group.com.chaxus.braid
Tunnel id:  com.chaxus.braid.tunnel
EOF
