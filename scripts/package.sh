#!/usr/bin/env bash
# Build an ad-hoc signed macOS .app via nsh (no Apple notarization).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

export PATH="${HOME}/.nsh/bin:${PATH}"
if ! command -v nsh >/dev/null 2>&1; then
  echo "nsh not found; install from https://nsh.dev/docs/installation" >&2
  exit 1
fi

OUT="${OUT:-.nsh/out}"
echo "Packaging Braid for macOS → ${OUT}"
nsh package --target macos -o "$OUT"
echo "Done. Look under ${OUT} for the .app (ad-hoc signed; not notarized)."
echo "Notarization / Developer ID is out of scope for the macOS MVP."
