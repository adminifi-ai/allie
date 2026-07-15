#!/bin/sh
# Generate a deterministic, relocatable checksum manifest for release archives.
set -eu

DIST=${1:-dist}
OUTPUT="$DIST/SHA256SUMS"
LC_ALL=C
export LC_ALL

if [ ! -d "$DIST" ]; then
  echo "release directory does not exist: $DIST" >&2
  exit 1
fi

set -- "$DIST"/*.tar.gz
if [ ! -f "$1" ]; then
  echo "no release archives found in $DIST" >&2
  exit 1
fi

TMP=$(mktemp "$DIST/.SHA256SUMS.XXXXXX")

checksum() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1"
  else
    shasum -a 256 "$1"
  fi
}

trap 'rm -f "$TMP"' EXIT HUP INT TERM
(
  cd "$DIST"
  for archive in ./*.tar.gz; do
    checksum "${archive#./}"
  done
) > "$TMP"
mv "$TMP" "$OUTPUT"
trap - EXIT HUP INT TERM

echo "Release checksums: $OUTPUT"
