#!/bin/sh
# Build a local Allie release bundle:
#   dist/allie-<host>.tar.gz
#
# The bundle layout is the runtime contract:
#   allie/bin/allie
#   allie/workers/browser/run.mjs
#   allie/node_modules/...
#   allie/ms-playwright/...
set -eu

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target_name=linux-x64 ;;
  Darwin-arm64) target_name=macos-arm64 ;;
  Darwin-x86_64) target_name=macos-x64 ;;
  *) target_name="$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)" ;;
esac

DIST="${1:-dist}"
BUNDLE="$DIST/allie"
ARCHIVE="$DIST/allie-$target_name.tar.gz"

rm -rf "$BUNDLE" "$ARCHIVE"
mkdir -p "$BUNDLE/bin" "$BUNDLE/workers" "$BUNDLE/fixtures"

if [ ! -d node_modules ]; then
  npm ci
fi

cargo build --release --locked
cp target/release/allie "$BUNDLE/bin/allie"
cp -R workers/browser "$BUNDLE/workers/browser"
cp -R fixtures/login "$BUNDLE/fixtures/login"
cp package.json "$BUNDLE/package.json"
cp package-lock.json "$BUNDLE/package-lock.json"
cp -R node_modules "$BUNDLE/node_modules"

PLAYWRIGHT_BROWSERS_PATH="$BUNDLE/ms-playwright" npx playwright install chromium
tar -czf "$ARCHIVE" -C "$DIST" allie
echo "Allie release bundle: $ARCHIVE"
