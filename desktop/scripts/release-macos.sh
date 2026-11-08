#!/bin/sh
# Build, sign and (optionally) notarize Keyfold for macOS.
#
#   npm run dmg                      (same as ./scripts/release-macos.sh)
#   KEYFOLD_NOTARY_PROFILE=keyfold-notary npm run dmg   (also notarizes)
#
# APPLE_SIGNING_IDENTITY  optional. Default: the first "Developer ID Application" identity in your login
#                         keychain (security find-identity -v -p codesigning).
# KEYFOLD_NOTARY_PROFILE  optional. A notarytool keychain profile created once with
#                         xcrun notarytool store-credentials <name> --apple-id <id> --team-id <team>
#                         Credentials stay in your keychain; nothing secret lives in the repo.
# KEYFOLD_UPDATER_KEY     optional. The private key that signs in-app updates, created once with
#                         npx tauri signer generate --ci -w ~/.tauri/keyfold-updater.key
#                         Default: ~/.tauri/keyfold-updater.key. Its public half is in tauri.conf.json;
#                         lose it and installed copies can no longer update themselves.
# KEYFOLD_RELEASE_NOTES   optional. A text file with what changed. Installed copies show it after
#                         updating; without it they only say the update was installed.
#
# Refuses to run unless tracked files have no local changes, src-tauri/Cargo.toml and package.json
# have the same version (scripts/check-version.sh) and HEAD carries the tag v<that version>.
# Untracked files (such as the release notes) are allowed.
#
# Leaves in target/release/bundle/updater/ what a GitHub release needs: the DMG (new installs),
# the signed app archive and latest.json (the in-app update reads it from the latest release).
set -eu

cd "$(dirname "$0")/.."

# A release must be exactly what the tag names: same version everywhere, no local edits.
VERSION=$(sh scripts/check-version.sh)
if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  echo "The working tree has uncommitted changes. Commit or stash them first." >&2
  git status --short --untracked-files=no >&2
  exit 65
fi
if ! git tag --points-at HEAD | grep -qx "v$VERSION"; then
  echo "HEAD is not tagged v$VERSION. Tag the release commit first: git tag v$VERSION" >&2
  exit 65
fi

# Without APPLE_SIGNING_IDENTITY, use the first Developer ID Application identity in the keychain,
# by its SHA-1 fingerprint (see the note on accents below).
if [ -z "${APPLE_SIGNING_IDENTITY:-}" ]; then
  APPLE_SIGNING_IDENTITY=$(security find-identity -v -p codesigning | awk '/"Developer ID Application/ { print $2; exit }')
  export APPLE_SIGNING_IDENTITY
fi
if [ -z "${APPLE_SIGNING_IDENTITY:-}" ]; then
  echo "No Developer ID Application identity found and APPLE_SIGNING_IDENTITY is not set. Available identities:" >&2
  security find-identity -v -p codesigning >&2
  exit 64
fi
# Accepts the identity name or its SHA-1 fingerprint. Prefer the fingerprint when the name
# has accents: codesign may not match a non-ASCII name passed through the environment.
if ! security find-identity -v -p codesigning | grep -F "$APPLE_SIGNING_IDENTITY" | grep -q "Developer ID Application"; then
  echo "Warning: '$APPLE_SIGNING_IDENTITY' is not a Developer ID Application identity; Gatekeeper will reject it outside your Mac." >&2
fi

UPDATER_KEY="${KEYFOLD_UPDATER_KEY:-$HOME/.tauri/keyfold-updater.key}"
if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
  if [ ! -f "$UPDATER_KEY" ]; then
    echo "No updater key at $UPDATER_KEY (see KEYFOLD_UPDATER_KEY above)." >&2
    exit 64
  fi
  TAURI_SIGNING_PRIVATE_KEY=$(cat "$UPDATER_KEY")
  export TAURI_SIGNING_PRIVATE_KEY
fi
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"

# Tauri signs the app (hardened runtime) and the DMG with APPLE_SIGNING_IDENTITY, and the update
# archive (Keyfold.app.tar.gz + .sig) with the updater key. Only release builds make the archive,
# so building from source needs no key.
npx tauri build --bundles app,dmg --config '{"bundle":{"createUpdaterArtifacts":true}}'

APP="target/release/bundle/macos/Keyfold.app"
DMG=$(ls -t target/release/bundle/dmg/Keyfold_*.dmg | head -1)

echo "Verifying signature..."
codesign --verify --deep --strict --verbose=2 "$APP"
codesign --verify --verbose=2 "$DMG"

if [ -n "${KEYFOLD_NOTARY_PROFILE:-}" ]; then
  echo "Notarizing $DMG ..."
  xcrun notarytool submit "$DMG" --keychain-profile "$KEYFOLD_NOTARY_PROFILE" --wait
  xcrun stapler staple "$DMG"
  spctl --assess --type open --context context:primary-signature --verbose=2 "$DMG"
else
  echo "Not notarized (KEYFOLD_NOTARY_PROFILE not set): Gatekeeper will warn on other Macs."
fi

case "$(uname -m)" in
  arm64) ARCH=aarch64 ;;
  *) ARCH=x86_64 ;;
esac
OUT="target/release/bundle/updater"
ARCHIVE="Keyfold_${VERSION}_${ARCH}.app.tar.gz"
rm -rf "$OUT"
mkdir -p "$OUT"
cp "$APP.tar.gz" "$OUT/$ARCHIVE"
cp "$DMG" "$OUT/"
NOTES_FILE="${KEYFOLD_RELEASE_NOTES:-}"
# node writes the JSON so quotes and line breaks in the notes are escaped.
VERSION="$VERSION" ARCH="$ARCH" ARCHIVE="$ARCHIVE" NOTES_FILE="$NOTES_FILE" SIG_FILE="$APP.tar.gz.sig" \
node -e '
const fs = require("fs")
const e = process.env
const notes = e.NOTES_FILE ? fs.readFileSync(e.NOTES_FILE, "utf8").trim() : ""
const latest = {
  version: e.VERSION,
  notes,
  pub_date: new Date().toISOString().replace(/\.\d+Z$/, "Z"),
  platforms: {
    ["darwin-" + e.ARCH]: {
      signature: fs.readFileSync(e.SIG_FILE, "utf8").trim(),
      url: `https://github.com/diegoperezg7/keyfold/releases/download/v${e.VERSION}/${e.ARCHIVE}`,
    },
  },
}
fs.writeFileSync(process.argv[1], JSON.stringify(latest, null, 2) + "\n")
' "$OUT/latest.json"

echo "Done: $OUT"
echo "Publish it (installed copies offer the update as soon as it is the latest release):"
if [ -n "$NOTES_FILE" ]; then
  echo "  gh release create v$VERSION $OUT/* --title \"Keyfold $VERSION\" --notes-file \"$NOTES_FILE\""
else
  echo "  gh release create v$VERSION $OUT/* --title \"Keyfold $VERSION\" --notes \"\""
fi
