#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
OUT="${GLYPHRAY_OUTPUT_DIR:-$ROOT/dist/macos}"
VERSION="${GLYPHRAY_VERSION:-$(tr -d '[:space:]' < "$ROOT/VERSION")}"
APP_NAME="GlyphRay Host"
APP_BUNDLE="$OUT/$APP_NAME.app"
CONTENTS="$APP_BUNDLE/Contents"
IDENTIFIER="app.glyphray.host"
APP_SIGNING_IDENTITY="${MACOS_APP_SIGNING_IDENTITY:--}"

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Version must use major.minor.patch form: $VERSION" >&2
  exit 2
fi

rm -rf "$APP_BUNDLE"
mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Resources" "$OUT"

cd "$ROOT/hosts/macos-host"
swift build -c release

cp ".build/release/GlyphRayMacHost" "$CONTENTS/MacOS/GlyphRayMacHost"
cp "$ROOT/tools/packaging/macos/Info.plist" "$CONTENTS/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $VERSION" "$CONTENTS/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $VERSION" "$CONTENTS/Info.plist"

if [[ "$APP_SIGNING_IDENTITY" == "-" ]]; then
  codesign --force --deep --sign - "$APP_BUNDLE"
else
  codesign --force --deep --options runtime --timestamp --sign "$APP_SIGNING_IDENTITY" "$APP_BUNDLE"
fi
codesign --verify --deep --strict --verbose=2 "$APP_BUNDLE"

PKG_PATH="$OUT/GlyphRayHost-$VERSION.pkg"
PKG_ARGS=(
  --component "$APP_BUNDLE"
  --identifier "$IDENTIFIER"
  --version "$VERSION"
  --install-location /Applications
)

if [[ -n "${MACOS_INSTALLER_SIGNING_IDENTITY:-}" ]]; then
  PKG_ARGS+=(--sign "$MACOS_INSTALLER_SIGNING_IDENTITY")
fi

pkgbuild "${PKG_ARGS[@]}" "$PKG_PATH"

if [[ -n "${MACOS_NOTARY_APPLE_ID:-}" && -n "${MACOS_NOTARY_TEAM_ID:-}" && -n "${MACOS_NOTARY_PASSWORD:-}" ]]; then
  xcrun notarytool submit "$PKG_PATH" \
    --apple-id "$MACOS_NOTARY_APPLE_ID" \
    --team-id "$MACOS_NOTARY_TEAM_ID" \
    --password "$MACOS_NOTARY_PASSWORD" \
    --wait
  xcrun stapler staple "$PKG_PATH"
  xcrun stapler validate "$PKG_PATH"
fi

ZIP_PATH="$OUT/GlyphRayHost-$VERSION-macos.zip"
rm -f "$ZIP_PATH"
ditto -c -k --sequesterRsrc --keepParent "$APP_BUNDLE" "$ZIP_PATH"

echo "Created $APP_BUNDLE"
echo "Created $PKG_PATH"
echo "Created $ZIP_PATH"
if [[ "$APP_SIGNING_IDENTITY" == "-" ]]; then
  echo "The app uses ad-hoc signing. Set MACOS_APP_SIGNING_IDENTITY for a distributable Developer ID signature."
fi
