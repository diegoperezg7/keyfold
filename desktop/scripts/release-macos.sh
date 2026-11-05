#!/bin/sh
# Build, sign and (optionally) notarize Keyfold for macOS.
#
#   APPLE_SIGNING_IDENTITY=<SHA-1 fingerprint or "Developer ID Application: Your Name (TEAMID)"> \
#   KEYFOLD_NOTARY_PROFILE=keyfold-notary \
#   ./scripts/release-macos.sh
#
# APPLE_SIGNING_IDENTITY  required. A "Developer ID Application" identity from your login
#                         keychain (security find-identity -v -p codesigning).
# KEYFOLD_NOTARY_PROFILE  optional. A notarytool keychain profile created once with
#                         xcrun notarytool store-credentials <name> --apple-id <id> --team-id <team>
#                         Credentials stay in your keychain; nothing secret lives in the repo.
set -eu

cd "$(dirname "$0")/.."

if [ -z "${APPLE_SIGNING_IDENTITY:-}" ]; then
  echo "APPLE_SIGNING_IDENTITY is not set. Available identities:" >&2
  security find-identity -v -p codesigning >&2
  exit 64
fi
# Accepts the identity name or its SHA-1 fingerprint. Prefer the fingerprint when the name
# has accents: codesign may not match a non-ASCII name passed through the environment.
if ! security find-identity -v -p codesigning | grep -F "$APPLE_SIGNING_IDENTITY" | grep -q "Developer ID Application"; then
  echo "Warning: '$APPLE_SIGNING_IDENTITY' is not a Developer ID Application identity; Gatekeeper will reject it outside your Mac." >&2
fi

# Tauri signs the app (hardened runtime) and the DMG with APPLE_SIGNING_IDENTITY.
npx tauri build --bundles app,dmg

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

echo "Done: $DMG"
