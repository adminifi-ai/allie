#!/bin/sh
# Module size gate (ratchet). Run from the repo root (as npm does).
#
# History: the lib.rs decomposition brought the largest Rust source file from
# 10055 lines down; extractions since (report.rs, agentic.rs, compliance.rs,
# model.rs, workbench.rs, worker.rs, standards.rs, release.rs, cli.rs,
# discovery.rs, ...) followed. This gate locks those wins in and covers every
# *.rs file under src/, including nested modules such as src/discovery/*.rs
# and src/workbench/tests.rs, not just the top-level src/*.rs glob.
#
# scripts/module-size-caps.tsv records each known file's cap. A file not
# listed there falls back to DEFAULT_CAP below, so a new module cannot
# silently balloon unnoticed just because no one added it to the table.
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
# Usage:
#   scripts/module-size-gate.sh              # run the gate against src/
#   scripts/module-size-gate.sh --self-test   # prove the gate's own logic
set -eu

CAPS_FILE=scripts/module-size-caps.tsv
DEFAULT_CAP=400

# cap_for <caps_file> <path>
# Prints the recorded cap for <path>, or nothing (exit 1) if unlisted.
cap_for() {
  awk -F'\t' -v p="$2" '
    $1 == p { print $2; found = 1; exit }
    END { if (!found) exit 1 }
  ' "$1"
}

# scan <root_dir> <caps_file>
# Walks every *.rs file under <root_dir>, checks it against its recorded cap
# (or DEFAULT_CAP if unlisted), and prints a FAIL line for every violation.
# Returns 1 if any file violates its cap.
scan() {
  root_dir=$1
  caps_file=$2
  status=0
  files=$(find "$root_dir" -name '*.rs' | sort)
  for f in $files; do
    lines=$(wc -l <"$f" | tr -d ' ')
    if cap=$(cap_for "$caps_file" "$f"); then
      origin="recorded cap"
    else
      cap=$DEFAULT_CAP
      origin="default cap"
    fi
    if [ "$lines" -gt "$cap" ]; then
      echo "FAIL: $f is $lines lines ($origin $cap). Extract a cohesive module instead of growing it, or deliberately raise this file's entry in scripts/module-size-caps.tsv and say why."
      status=1
    fi
  done
  return "$status"
}

self_test() {
  test_file="src/discovery/__module_size_gate_selftest__.rs"
  tmp_caps=$(mktemp)
  out=$(mktemp)
  trap 'rm -f "$test_file" "$tmp_caps" "$out"' EXIT INT TERM

  failures=0
  echo "== module-size-gate self-test =="

  # 0. Baseline: the real tree, unmodified, must already be green.
  if scan src "$CAPS_FILE" >"$out" 2>&1; then
    echo "PASS: real src/ tree is green before injection"
  else
    echo "FAIL: real src/ tree is not green before injection:"
    cat "$out"
    failures=$((failures + 1))
  fi

  # 1. Recursive scan + default-cap path: an unlisted file nested under a
  #    subdirectory (src/discovery/) that exceeds DEFAULT_CAP must be caught.
  #    This also proves the scan is recursive, not just top-level src/*.rs.
  over_default=$((DEFAULT_CAP + 5))
  awk -v n="$over_default" 'BEGIN { for (i = 0; i < n; i++) print "// selftest line" }' >"$test_file"
  if scan src "$CAPS_FILE" >"$out" 2>&1; then
    echo "FAIL: gate did not catch a nested, unlisted file exceeding DEFAULT_CAP ($DEFAULT_CAP)"
    failures=$((failures + 1))
  elif grep -qF "$test_file is $over_default lines (default cap $DEFAULT_CAP)" "$out"; then
    echo "PASS: nested unlisted file over DEFAULT_CAP is caught, naming file/size/cap"
  else
    echo "FAIL: violation message missing expected file/size/cap detail:"
    cat "$out"
    failures=$((failures + 1))
  fi
  rm -f "$test_file"

  # 2. Explicit per-file cap path: a file recorded with a tight cap must be
  #    caught even though it stays well under DEFAULT_CAP.
  tight_cap=5
  over_tight=$((tight_cap + 3))
  cp "$CAPS_FILE" "$tmp_caps"
  printf '%s\t%s\n' "$test_file" "$tight_cap" >>"$tmp_caps"
  awk -v n="$over_tight" 'BEGIN { for (i = 0; i < n; i++) print "// selftest line" }' >"$test_file"
  if scan src "$tmp_caps" >"$out" 2>&1; then
    echo "FAIL: gate did not catch a file exceeding its recorded per-file cap"
    failures=$((failures + 1))
  elif grep -qF "$test_file is $over_tight lines (recorded cap $tight_cap)" "$out"; then
    echo "PASS: file over its recorded per-file cap is caught, naming file/size/cap"
  else
    echo "FAIL: violation message missing expected file/size/cap detail:"
    cat "$out"
    failures=$((failures + 1))
  fi
  rm -f "$test_file"

  # 3. Cleanup confirmation: once the injected file is gone, the real tree
  #    is green again (proves the gate isn't stuck red from the test itself).
  if scan src "$CAPS_FILE" >"$out" 2>&1; then
    echo "PASS: real src/ tree is green again after cleanup"
  else
    echo "FAIL: real src/ tree is not green after cleanup:"
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

if scan src "$CAPS_FILE"; then
  echo "module-size gate ok: every src/**/*.rs file is within its recorded (or default $DEFAULT_CAP-line) cap."
  exit 0
fi
exit 1
