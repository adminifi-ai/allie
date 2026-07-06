#!/bin/sh
# Module size gate (ratchet).
#
# History: the lib.rs decomposition brought the largest Rust source file from
# 10055 lines down; extractions since (report.rs, agentic.rs, compliance.rs,
# model.rs, workbench.rs, worker.rs, standards.rs, release.rs, cli.rs,
# discovery.rs, ...) followed. This gate locks those wins in and covers every
# *.rs file in the repo (excluding target/, node_modules/, .git/), not just
# the top-level src/*.rs glob it used to check — that includes nested
# modules such as src/discovery/*.rs and src/workbench/tests.rs today, and
# any Rust source that lands outside src/ in the future.
#
# scripts/module-size-caps.tsv records each known file's cap, keyed by its
# path relative to the repo root. A file not listed there falls back to
# DEFAULT_CAP below, so a new module cannot silently balloon unnoticed just
# because no one added it to the table.
#
# Ratchet discipline:
#   - Shrinking is free: once a file's size drops, lower its entry in
#     module-size-caps.tsv in the same change (or a fast follow-up) to lock
#     the win in. A lower cap never fails a file that is still shrinking.
#   - Growing past a cap requires a deliberate, reviewed decision: extract a
#     cohesive module instead. If growth is truly unavoidable, raise ONLY
#     that file's line in module-size-caps.tsv and say why in the commit/PR
#     — never raise DEFAULT_CAP, and never raise a cap silently.
#
# Correctness invariants this script holds itself to (see --self-test):
#   - Filenames containing spaces or other shell-special characters must be
#     handled as single, whole paths (`IFS= read -r` line iteration, never
#     unquoted `for f in $files` word-splitting). This assumes no filename
#     contains a literal newline, which is true in practice and lets the
#     rest of the script stay portable /bin/sh (no `read -d`/`-print0`,
#     which are bash/GNU extensions undefined in POSIX sh).
#   - The scan always resolves the repo root from the script's own location,
#     never the caller's CWD, so `cd scripts && ./module-size-gate.sh` and
#     `./scripts/module-size-gate.sh` from the repo root behave identically.
#   - Scanning zero files is always a loud failure, never a silent "ok" —
#     a healthy run reports how many files it scanned.
#   - Line counts include a final line with no trailing newline (wc -l does
#     not; this script deliberately avoids it for that reason).
#
# Usage:
#   scripts/module-size-gate.sh              # run the gate against the repo
#   scripts/module-size-gate.sh --self-test   # prove the gate's own logic
set -eu

CDPATH=
SCRIPT_DIR=$(cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/.." && pwd)
CAPS_FILE="$SCRIPT_DIR/module-size-caps.tsv"
DEFAULT_CAP=400

CLEANUP_MANIFEST=$(mktemp)
# shellcheck disable=SC2329 # invoked indirectly by the trap below
cleanup() {
  if [ -f "$CLEANUP_MANIFEST" ]; then
    while IFS= read -r p; do rm -rf -- "$p"; done <"$CLEANUP_MANIFEST"
  fi
  rm -f -- "$CLEANUP_MANIFEST"
}
trap cleanup EXIT INT TERM

# add_cleanup <path>
# Registers <path> (file or directory) for removal on exit. Newline-
# delimited so paths containing spaces are handled correctly.
add_cleanup() {
  printf '%s\n' "$1" >>"$CLEANUP_MANIFEST"
}

LIST_FILE=$(mktemp)
add_cleanup "$LIST_FILE"

# cap_for <caps_file> <path>
# Prints the recorded cap for <path>, or nothing (exit 1) if unlisted.
cap_for() {
  awk -F'\t' -v p="$2" '
    $1 == p { print $2; found = 1; exit }
    END { if (!found) exit 1 }
  ' "$1"
}

# count_lines <path>
# Prints the number of lines in <path>, counting a final line with no
# trailing newline as one more line (unlike `wc -l`, which only counts
# newline characters and would silently undercount such a file).
count_lines() {
  awk 'END { print NR }' "$1"
}

# scan <root_dir> <caps_file>
# Walks every *.rs file under <root_dir> (excluding target/, node_modules/,
# .git/), checks it against its recorded cap (or DEFAULT_CAP if unlisted),
# and prints a FAIL line for every violation. Sets SCAN_COUNT to the number
# of files examined. Returns 1 if any file violates its cap, or if zero
# files were found — a healthy run must find files, never look green by
# accident.
scan() {
  root_dir=$1
  caps_file=$2
  status=0
  scanned=0

  find "$root_dir" \( -name target -o -name node_modules -o -name .git \) -prune -o \
    -type f -name '*.rs' -print >"$LIST_FILE" || true

  while IFS= read -r abs; do
    scanned=$((scanned + 1))
    relpath=${abs#"$root_dir"/}
    lines=$(count_lines "$abs")
    if cap=$(cap_for "$caps_file" "$relpath"); then
      origin="recorded cap"
    else
      cap=$DEFAULT_CAP
      origin="default cap"
    fi
    if [ "$lines" -gt "$cap" ]; then
      echo "FAIL: $relpath is $lines lines ($origin $cap). Extract a cohesive module instead of growing it, or deliberately raise this file's entry in scripts/module-size-caps.tsv and say why."
      status=1
    fi
  done <"$LIST_FILE"

  SCAN_COUNT=$scanned
  if [ "$scanned" -eq 0 ]; then
    echo "FAIL: scanned 0 *.rs files under $root_dir. A healthy run must find files — this usually means the gate was pointed at the wrong root. Treat zero files found as a failure, never as green."
    return 1
  fi

  return "$status"
}

self_test() {
  space_file="$REPO_ROOT/src/discovery/temp file with space.rs"
  no_newline_file="$REPO_ROOT/src/discovery/__module_size_gate_selftest_no_newline__.rs"
  empty_dir=$(mktemp -d)
  add_cleanup "$space_file"
  add_cleanup "$no_newline_file"
  add_cleanup "$empty_dir"
  out=$(mktemp)
  add_cleanup "$out"

  failures=0
  echo "== module-size-gate self-test =="

  # 0. Baseline: the real tree, unmodified, must already be green and report
  #    a nonzero scanned-file count (self-evidencing, not just silent "ok").
  if scan "$REPO_ROOT" "$CAPS_FILE" >"$out" 2>&1; then
    baseline_count=$SCAN_COUNT
    if [ "$baseline_count" -gt 0 ]; then
      echo "PASS: real repo tree is green before injection ($baseline_count files scanned)"
    else
      echo "FAIL: real repo tree reported 0 files scanned yet claimed success"
      failures=$((failures + 1))
    fi
  else
    echo "FAIL: real repo tree is not green before injection:"
    cat "$out"
    failures=$((failures + 1))
  fi

  # 1. Filenames containing spaces must be handled as whole paths, not
  #    word-split. An unlisted, over-DEFAULT_CAP file with a space in its
  #    name, nested under src/discovery/, must be caught.
  over_default=$((DEFAULT_CAP + 10))
  awk -v n="$over_default" 'BEGIN { for (i = 0; i < n; i++) print "// selftest line" }' >"$space_file"
  if scan "$REPO_ROOT" "$CAPS_FILE" >"$out" 2>&1; then
    echo "FAIL: gate did not catch a nested, unlisted file with a space in its name exceeding DEFAULT_CAP ($DEFAULT_CAP)"
    failures=$((failures + 1))
  elif grep -qF "src/discovery/temp file with space.rs is $over_default lines (default cap $DEFAULT_CAP)" "$out"; then
    echo "PASS: filename containing a space is scanned as one path and its cap violation is caught"
  else
    echo "FAIL: violation message missing expected file/size/cap detail for the space-in-name file:"
    cat "$out"
    failures=$((failures + 1))
  fi
  rm -f -- "$space_file"

  # 2. Scanning an empty directory must fail loudly, never report green.
  #    This is the zero-file guard: a wrong root (e.g. a caller-relative
  #    path resolved against the wrong CWD) must never look like a pass.
  if scan "$empty_dir" "$CAPS_FILE" >"$out" 2>&1; then
    echo "FAIL: scanning an empty directory reported success instead of failing loud"
    failures=$((failures + 1))
  elif grep -qF "scanned 0 *.rs files under $empty_dir" "$out"; then
    echo "PASS: scanning zero files fails loud with a clear message"
  else
    echo "FAIL: zero-file failure message missing expected detail:"
    cat "$out"
    failures=$((failures + 1))
  fi

  # 2b. The gate resolves its root from its own location, not the caller's
  #     CWD: invoking it from an unrelated directory must still scan the
  #     whole repo and report the same nonzero file count.
  cwd_out=$(cd /tmp && "$SCRIPT_DIR/module-size-gate.sh") && cwd_status=0 || cwd_status=$?
  if [ "$cwd_status" -ne 0 ]; then
    echo "FAIL: invoking the gate from /tmp (unrelated CWD) did not exit 0:"
    echo "$cwd_out"
    failures=$((failures + 1))
  elif echo "$cwd_out" | grep -qE "^module-size gate ok: $baseline_count \*\.rs files scanned"; then
    echo "PASS: invoking the gate from an unrelated CWD (/tmp) still scans the full repo ($baseline_count files)"
  else
    echo "FAIL: invoking the gate from /tmp did not report the expected scanned-file count:"
    echo "$cwd_out"
    failures=$((failures + 1))
  fi

  # 3. A file whose final line has no trailing newline must still be
  #    counted correctly. `wc -l` undercounts such a file by one line
  #    (it counts newline characters, not lines); this gate must not.
  awk -v n="$DEFAULT_CAP" 'BEGIN {
    for (i = 0; i < n; i++) printf "// selftest line\n"
    printf "// selftest final line without a trailing newline"
  }' >"$no_newline_file"
  expected_lines=$((DEFAULT_CAP + 1))
  if scan "$REPO_ROOT" "$CAPS_FILE" >"$out" 2>&1; then
    echo "FAIL: gate did not catch a file over cap whose final line lacks a trailing newline (wc -l would have undercounted it by one)"
    failures=$((failures + 1))
  elif grep -qF "src/discovery/__module_size_gate_selftest_no_newline__.rs is $expected_lines lines (default cap $DEFAULT_CAP)" "$out"; then
    echo "PASS: a file whose final line lacks a trailing newline is counted correctly ($expected_lines lines, not undercounted)"
  else
    echo "FAIL: violation message missing expected file/size/cap detail for the no-trailing-newline file:"
    cat "$out"
    failures=$((failures + 1))
  fi
  rm -f -- "$no_newline_file"

  # 4. Cleanup confirmation: once every injected file is gone, the real
  #    tree is green again with the same scanned-file count as baseline.
  if scan "$REPO_ROOT" "$CAPS_FILE" >"$out" 2>&1; then
    if [ "$SCAN_COUNT" -eq "$baseline_count" ]; then
      echo "PASS: real repo tree is green again after cleanup ($SCAN_COUNT files scanned)"
    else
      echo "FAIL: scanned file count after cleanup ($SCAN_COUNT) does not match baseline ($baseline_count)"
      failures=$((failures + 1))
    fi
  else
    echo "FAIL: real repo tree is not green after cleanup:"
    cat "$out"
    failures=$((failures + 1))
  fi

  if [ "$failures" -eq 0 ]; then
    echo "module-size-gate self-test: all checks passed"
    return 0
  fi
  echo "module-size-gate self-test: $failures check(s) failed"
  return 1
}

if [ "${1:-}" = "--self-test" ]; then
  self_test
  exit $?
fi

if scan "$REPO_ROOT" "$CAPS_FILE"; then
  echo "module-size gate ok: $SCAN_COUNT *.rs files scanned across the repo (excluding target/, node_modules/, .git/); every file is within its recorded (or default $DEFAULT_CAP-line) cap."
  exit 0
fi
exit 1
