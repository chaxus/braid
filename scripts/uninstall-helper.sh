#!/usr/bin/env bash
# Remove braid-helper LaunchDaemon.
set -euo pipefail

LABEL="com.chaxus.braid.helper"
HELPER_DST="/Library/PrivilegedHelperTools/${LABEL}"
PLIST_DST="/Library/LaunchDaemons/${LABEL}.plist"
SOCK="/var/run/braid-helper.sock"

if [[ "$(id -u)" -ne 0 ]]; then
  exec sudo "$0" "$@"
fi

launchctl bootout system/"$LABEL" 2>/dev/null || true
rm -f "$PLIST_DST" "$HELPER_DST" "$SOCK"
echo "Removed ${LABEL}."
