#!/usr/bin/env bash
# Builds, signs and (optionally) publishes a Dilo release that installed apps can update to from inside the app.
#
#   ./scripts/release.sh            build + prepare release/ (Dilo.dmg, Dilo.app.tar.gz, .sig, latest.json)
#   ./scripts/release.sh --publish  the same, then create the GitHub release for the version in tauri.conf.json
#
# Needs: the updater key at ~/.tauri/dilo-updater.key (never commit it) and the code-signing identity that
# scripts/build-local.sh uses. Keep BOTH the same for every release: macOS ties the permissions the user
# granted to that signature, and the updater rejects updates not signed with the matching key.
# Release notes: put them in release-notes.md (markdown) before running.
set -euo pipefail
cd "$(dirname "$0")/.."
KEY="$HOME/.tauri/dilo-updater.key"
[ -f "$KEY" ] || { echo "Falta la llave de firma $KEY" >&2; exit 1; }
VERSION="$(python3 -c "import json;print(json.load(open('src-tauri/tauri.conf.json'))['version'])")"
TAG="v$VERSION"
# The three version numbers must agree, otherwise the updater/manifest ship the wrong version.
CARGO_V="$(sed -n 's/^version = "\(.*\)"/\1/p' src-tauri/Cargo.toml | head -n1)"
PKG_V="$(python3 -c "import json;print(json.load(open('package.json'))['version'])")"
if [ "$CARGO_V" != "$VERSION" ] || [ "$PKG_V" != "$VERSION" ]; then
  echo "Versiones distintas: tauri.conf=$VERSION Cargo.toml=$CARGO_V package.json=$PKG_V" >&2; exit 1
fi
if [ "${1:-}" = "--publish" ] && git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
  echo "El tag $TAG ya existe; sube la versión." >&2; exit 1
fi
if [ -f release-notes.md ] && ! grep -q "$VERSION" release-notes.md; then
  echo "Ojo: release-notes.md no menciona $VERSION; ¿son notas viejas?" >&2
fi
export TAURI_SIGNING_PRIVATE_KEY="$KEY" TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
export PATH="$HOME/.cargo/bin:$PATH"
PREFERRED="SonarLab Local Development"
IDENTITIES="$(security find-identity -v -p codesigning | awk -F'"' 'NF>1{print $2}')"
if grep -qxF "$PREFERRED" <<<"$IDENTITIES"; then IDENTITY="$PREFERRED"; else IDENTITY="$(head -n1 <<<"$IDENTITIES")"; fi
[ -n "$IDENTITY" ] || { echo "No hay identidad de firma de código." >&2; exit 1; }
echo "Versión $VERSION, firmando con: $IDENTITY"
APPLE_SIGNING_IDENTITY="$IDENTITY" bun tauri build --bundles app

BUNDLE=src-tauri/target/release/bundle/macos
rm -rf release && mkdir -p release/stage
ditto "$BUNDLE/Dilo.app" release/stage/Dilo.app
ln -s /Applications release/stage/Applications
hdiutil create -volname "Dilo" -srcfolder release/stage -ov -format UDZO -fs HFS+ release/Dilo.dmg >/dev/null
rm -rf release/stage
cp "$BUNDLE/Dilo.app.tar.gz" "$BUNDLE/Dilo.app.tar.gz.sig" release/
NOTES=-; [ -f release-notes.md ] && NOTES=release-notes.md
python3 scripts/make-manifest.py "$VERSION" "https://github.com/marvalre/dilo/releases/download/$TAG" "$NOTES" release/latest.json
echo "SHA-256 Dilo.dmg: $(shasum -a 256 release/Dilo.dmg | cut -d' ' -f1)"

if [ "${1:-}" = "--publish" ]; then
  [ "$NOTES" = "-" ] && echo "Ojo: no hay release-notes.md; GitHub generará las notas."
  NOTES_ARGS=(--generate-notes); [ "$NOTES" != "-" ] && NOTES_ARGS=(--notes-file "$NOTES")
  gh release create "$TAG" release/Dilo.dmg release/Dilo.app.tar.gz release/Dilo.app.tar.gz.sig release/latest.json \
    --repo marvalre/dilo --title "Dilo $VERSION" "${NOTES_ARGS[@]}"
  echo "Publicada: https://github.com/marvalre/dilo/releases/tag/$TAG"
else
  echo "Listo en release/. Para publicar: ./scripts/release.sh --publish"
fi
