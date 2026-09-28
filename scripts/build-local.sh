#!/usr/bin/env bash
# Local macOS build signed with a stable local identity, so macOS keeps the
# Accessibility/Microphone grants across rebuilds (ad-hoc signatures change every build).
# Usage: ./scripts-build-local.sh ["Signing Identity Name"]
set -euo pipefail
cd "$(dirname "$0")/.."
IDENTITY="${1:-$(security find-identity -v -p codesigning | awk -F'"' 'NR==1{print $2}')}"
echo "Signing with: ${IDENTITY:-ad-hoc}"
export PATH="$HOME/.cargo/bin:$PATH"
APPLE_SIGNING_IDENTITY="$IDENTITY" bun tauri build --bundles app
