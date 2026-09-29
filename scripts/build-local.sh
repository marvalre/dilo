#!/usr/bin/env bash
# Local macOS build signed with a stable local identity, so macOS keeps the
# Accessibility/Microphone grants across rebuilds (ad-hoc signatures change every build).
# Usage: ./scripts/build-local.sh ["Signing Identity Name"]
set -euo pipefail
cd "$(dirname "$0")/.."
PREFERRED="SonarLab Local Development"
IDENTITIES="$(security find-identity -v -p codesigning | awk -F'"' 'NF>1{print $2}')"
if [ -n "${1:-}" ]; then
  IDENTITY="$1"
elif grep -qxF "$PREFERRED" <<<"$IDENTITIES"; then
  IDENTITY="$PREFERRED"
else
  IDENTITY="$(head -n1 <<<"$IDENTITIES")"
fi
if [ -z "$IDENTITY" ]; then
  echo "No code-signing identity found. Without one macOS forgets permissions on every build." >&2
  exit 1
fi
echo "Signing with: $IDENTITY"
export PATH="$HOME/.cargo/bin:$PATH"
APPLE_SIGNING_IDENTITY="$IDENTITY" bun tauri build --bundles app
