#!/bin/sh
set -eu

CDPATH=''
ROOT=$(cd -- "$(dirname "$0")/.." && pwd)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT HUP INT TERM

mkdir -p "$TMP/dist"
printf 'linux bundle\n' > "$TMP/dist/allie-linux-x64.tar.gz"
printf 'macOS bundle\n' > "$TMP/dist/allie-macos-arm64.tar.gz"

"$ROOT/scripts/release-checksums.sh" "$TMP/dist"

test -f "$TMP/dist/SHA256SUMS"
test "$(wc -l < "$TMP/dist/SHA256SUMS" | tr -d ' ')" -eq 2
test "$(grep -c '  allie-linux-x64\.tar\.gz$' "$TMP/dist/SHA256SUMS")" -eq 1
test "$(grep -c '  allie-macos-arm64\.tar\.gz$' "$TMP/dist/SHA256SUMS")" -eq 1
if grep -q "$TMP" "$TMP/dist/SHA256SUMS"; then
  echo "FAIL: SHA256SUMS must contain stable basenames, not build paths" >&2
  exit 1
fi

(cd "$TMP/dist" && sha256sum --check SHA256SUMS)
printf 'tampered\n' >> "$TMP/dist/allie-linux-x64.tar.gz"
if (cd "$TMP/dist" && sha256sum --check SHA256SUMS >/dev/null 2>&1); then
  echo "FAIL: checksum verification accepted a tampered archive" >&2
  exit 1
fi

echo "release integrity smoke passed"
