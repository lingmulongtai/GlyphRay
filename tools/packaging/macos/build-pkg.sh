#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
OUT="$ROOT/dist/macos"
PAYLOAD="$OUT/payload"
IDENTIFIER="app.glyphray.host"
VERSION="0.1.0"

mkdir -p "$PAYLOAD/usr/local/bin" "$OUT"

cd "$ROOT/hosts/macos-host"
swift build -c release

cp ".build/release/GlyphRayMacHost" "$PAYLOAD/usr/local/bin/glyphray-macos-host"

pkgbuild \
  --root "$PAYLOAD" \
  --identifier "$IDENTIFIER" \
  --version "$VERSION" \
  --install-location "/" \
  "$OUT/GlyphRayHost-$VERSION.pkg"

echo "Created $OUT/GlyphRayHost-$VERSION.pkg"
