#!/bin/sh
# Module size gate (ratchet).
#
# History: the lib.rs decomposition brought the largest Rust source file from
# 10055 lines down; extractions since (report.rs, agentic.rs, compliance.rs,
# model.rs, workbench.rs, worker.rs, standards.rs, release.rs, cli.rs,
# discovery.rs, ...) followed. This gate locks those wins in and covers every
# tracked or nonignored untracked *.rs file in the canonical checkout, not just
# the top-level src/*.rs glob it used to check. That includes nested modules
# such as src/discovery/*.rs and src/workbench/tests.rs today, plus Rust sources
# outside src/. Ignored caches, generated output, and nested worktrees are not
# part of the canonical checkout's source set; tracked files remain in scope
# regardless of directory name.
#
# scripts/module-size-caps.tsv records each known file's cap, keyed by its
# path relative to the repo root. The canonical checkout scan follows Git's
# tracked + nonignored-untracked file view, so ignored caches and nested
# worktrees cannot leak into the result. A file not listed there falls back to
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
#   - Filenames containing spaces must be handled as single, whole paths
#     (`IFS= read -r` line iteration, never unquoted `for f in $files`
#     word-splitting). Control characters, quotes, backslashes, and literal
#     newlines fail closed as unreadable Git-quoted paths; supporting them
#     transparently would require non-POSIX NUL-delimited reads.
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
# Prints the recorded cap for <path>, or nothing if unlisted. I/O failure stays
# nonzero so callers cannot confuse a missing policy file with a default cap.
cap_for() {
  awk -F'\t' -v p="$2" '
    $1 == p { print $2; found = 1; exit }
  ' "$1"
}

# validate_caps_file <caps_file>
# Refuses missing, empty, malformed, duplicate, or nonnumeric policy before any
# source scan. Defaults apply only to paths absent from a valid policy file.
validate_caps_file() {
  caps_file=$1
  if [ ! -r "$caps_file" ]; then
    echo "FAIL: module-size caps policy is not readable: $caps_file"
    return 1
  fi
  if ! awk -F'\t' '
    /^[[:space:]]*($|#)/ { next }
    NF != 2 || $1 == "" || $2 !~ /^[0-9]+$/ || seen[$1]++ { bad = 1 }
    { rules += 1 }
    END { if (bad || rules == 0) exit 1 }
  ' "$caps_file"; then
    echo "FAIL: module-size caps policy is empty or malformed: $caps_file"
    return 1
  fi
}

# count_lines <path>
# Prints the number of lines in <path>, counting a final line with no
# trailing newline as one more line (unlike `wc -l`, which only counts
# newline characters and would silently undercount such a file).
count_lines() {
  awk 'END { print NR }' "$1"
}

# list_rust_files <root_dir>
# Writes repository-relative Rust source paths to LIST_FILE. Git owns file
# visibility: tracked files plus nonignored untracked files are included, while
# ignored output and nested worktrees are excluded. Enumeration fails closed;
# a partial list is never accepted.
list_rust_files() {
  if ! list_root=$(cd -- "$1" 2>/dev/null && pwd -P); then
    echo "FAIL: cannot resolve scan root $1"
    return 1
  fi

  if ! repo_top=$(git -C "$list_root" rev-parse --show-toplevel 2>/dev/null); then
    echo "FAIL: cannot enumerate Rust sources: $list_root is not a readable Git worktree"
    return 1
  fi
  if [ "$repo_top" != "$list_root" ]; then
    echo "FAIL: scan root $list_root is not the Git worktree root $repo_top"
    return 1
  fi

  if ! git -c core.quotePath=false -C "$list_root" \
    ls-files --cached --others --exclude-standard -- '*.rs' >"$LIST_FILE"; then
    echo "FAIL: Git Rust-source enumeration failed under $list_root; refusing a partial scan"
    return 1
  fi
}

# scan <root_dir> <caps_file>
# Checks every Rust source selected by list_rust_files against its recorded cap
# (or DEFAULT_CAP if unlisted),
# and prints a FAIL line for every violation. Sets SCAN_COUNT to the number
# of files examined. Returns 1 if any file violates its cap, or if zero
# files were found — a healthy run must find files, never look green by
# accident.
scan() {
  scan_root=$1
  caps_file=$2
  status=0
  scanned=0

  if ! validate_caps_file "$caps_file"; then
    return 1
  fi
  if ! list_rust_files "$scan_root"; then
    return 1
  fi

  while IFS= read -r relpath; do
    scanned=$((scanned + 1))
    abs="$scan_root/$relpath"
    if ! lines=$(count_lines "$abs" 2>/dev/null); then
      echo "FAIL: Git listed $relpath but the gate could not read it; refusing to skip a source file"
      status=1
      continue
    fi
    case "$lines" in
      '' | *[!0-9]*)
        echo "FAIL: could not determine a numeric line count for $relpath"
        status=1
        continue
        ;;
    esac
    if ! cap=$(cap_for "$caps_file" "$relpath"); then
      echo "FAIL: could not read module-size cap policy for $relpath"
      status=1
      continue
    elif [ -n "$cap" ]; then
      origin="recorded cap"
    else
      cap=$DEFAULT_CAP
      origin="default cap"
    fi
    case "$cap" in
      '' | *[!0-9]*)
        echo "FAIL: $relpath has an invalid non-numeric cap: $cap"
        status=1
        continue
        ;;
    esac
    if [ "$lines" -gt "$cap" ]; then
      echo "FAIL: $relpath is $lines lines ($origin $cap). Extract a cohesive module instead of growing it, or deliberately raise this file's entry in scripts/module-size-caps.tsv and say why."
      status=1
    fi
  done <"$LIST_FILE"

  SCAN_COUNT=$scanned
  if [ "$scanned" -eq 0 ]; then
    echo "FAIL: scanned 0 *.rs files under $scan_root. A healthy run must find files — this usually means the gate was pointed at the wrong root. Treat zero files found as a failure, never as green."
    return 1
  fi

  return "$status"
}

self_test() {
  fixture_repo=$(mktemp -d)
  empty_repo=$(mktemp -d)
  fake_bin=$(mktemp -d)
  add_cleanup "$fixture_repo"
  add_cleanup "$empty_repo"
  add_cleanup "$fake_bin"
  out=$(mktemp)
  add_cleanup "$out"

  failures=0
  echo "== module-size-gate self-test =="

  fixture_git() {
    env GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
      git -c core.hooksPath=/dev/null -c commit.gpgsign=false \
      -C "$fixture_repo" "$@"
  }

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

  # Build every destructive fixture in a unique temporary Git repository.
  # The real checkout is read-only throughout the self-test.
  fixture_git init -q
  mkdir -p "$fixture_repo/src"
  printf '.ignored/\n.worktrees/\n!src/\n!src/*.rs\n' >"$fixture_repo/.gitignore"
  printf '// tracked fixture\n' >"$fixture_repo/src/tracked.rs"
  fixture_git add .gitignore src/tracked.rs
  fixture_git -c user.name=allie-self-test \
    -c user.email=allie-self-test@example.invalid commit -qm 'module-size fixture'
  printf '// nonignored untracked fixture\n' >"$fixture_repo/src/untracked.rs"

  if scan "$fixture_repo" "$CAPS_FILE" >"$out" 2>&1 && [ "$SCAN_COUNT" -eq 2 ]; then
    fixture_count=$SCAN_COUNT
    echo "PASS: fixture baseline includes tracked and nonignored untracked Rust files ($fixture_count files)"
  else
    echo "FAIL: fixture baseline did not include exactly its tracked and nonignored untracked Rust files:"
    cat "$out"
    failures=$((failures + 1))
    fixture_count=2
  fi

  # 1. A tracked file over cap must fail. This is independent of ambient repo
  #    contents and proves the index half of the enumeration contract.
  over_default=$((DEFAULT_CAP + 10))
  awk -v n="$over_default" 'BEGIN { for (i = 0; i < n; i++) print "// tracked line" }' >"$fixture_repo/src/tracked.rs"
  if scan "$fixture_repo" "$CAPS_FILE" >"$out" 2>&1; then
    echo "FAIL: gate did not catch an oversized tracked Rust file"
    failures=$((failures + 1))
  elif grep -qF "src/tracked.rs is $over_default lines (default cap $DEFAULT_CAP)" "$out"; then
    echo "PASS: oversized tracked Rust files fail the gate"
  else
    echo "FAIL: tracked-file violation lacked expected path/size/cap detail:"
    cat "$out"
    failures=$((failures + 1))
  fi
  printf '// tracked fixture\n' >"$fixture_repo/src/tracked.rs"

  # 2. A nonignored untracked file with a space must remain one path and fail.
  space_file="$fixture_repo/src/temp file with space.rs"
  awk -v n="$over_default" 'BEGIN { for (i = 0; i < n; i++) print "// untracked line" }' >"$space_file"
  if scan "$fixture_repo" "$CAPS_FILE" >"$out" 2>&1; then
    echo "FAIL: gate did not catch an oversized nonignored untracked Rust file with a space"
    failures=$((failures + 1))
  elif grep -qF "src/temp file with space.rs is $over_default lines (default cap $DEFAULT_CAP)" "$out"; then
    echo "PASS: nonignored untracked filenames containing spaces are scanned as one path"
  else
    echo "FAIL: untracked space-in-name violation lacked expected detail:"
    cat "$out"
    failures=$((failures + 1))
  fi
  rm -f -- "$space_file"

  # 3. Ignored files and a real linked worktree under an ignored directory
  #    must not contaminate the source set.
  mkdir -p "$fixture_repo/.ignored"
  awk -v n="$over_default" 'BEGIN { for (i = 0; i < n; i++) print "// ignored line" }' >"$fixture_repo/.ignored/oversized.rs"
  linked_worktree="$fixture_repo/.worktrees/linked"
  if fixture_git worktree add --detach "$linked_worktree" HEAD >/dev/null 2>&1; then
    awk -v n="$over_default" 'BEGIN { for (i = 0; i < n; i++) print "// linked worktree line" }' >"$linked_worktree/src/oversized.rs"
    if scan "$fixture_repo" "$CAPS_FILE" >"$out" 2>&1 && [ "$SCAN_COUNT" -eq "$fixture_count" ]; then
      echo "PASS: ignored files and a real ignored linked worktree do not contaminate the scan"
    else
      echo "FAIL: ignored files or linked worktree contaminated the source scan:"
      cat "$out"
      failures=$((failures + 1))
    fi
    fixture_git worktree remove --force "$linked_worktree" >/dev/null 2>&1 || true
  else
    echo "FAIL: could not create the linked-worktree self-test fixture"
    failures=$((failures + 1))
  fi

  # 4. A tracked path that disappears after enumeration must fail closed,
  #    never count as scanned and then silently green.
  rm -f -- "$fixture_repo/src/tracked.rs"
  if scan "$fixture_repo" "$CAPS_FILE" >"$out" 2>&1; then
    echo "FAIL: gate skipped a missing tracked Rust file and returned success"
    failures=$((failures + 1))
  elif grep -qF "Git listed src/tracked.rs but the gate could not read it" "$out"; then
    echo "PASS: unreadable or missing listed Rust files fail closed"
  else
    echo "FAIL: missing tracked file did not produce the fail-closed diagnostic:"
    cat "$out"
    failures=$((failures + 1))
  fi
  printf '// tracked fixture\n' >"$fixture_repo/src/tracked.rs"

  # 5. A Git enumeration that emits a partial list and exits nonzero must fail.
  real_git=$(command -v git)
  # shellcheck disable=SC2016 # these are literal lines of the fake git script
  printf '%s\n' \
    '#!/bin/sh' \
    'for arg do' \
    '  if [ "$arg" = "ls-files" ]; then' \
    '    printf "%s\\n" src/tracked.rs' \
    '    exit 42' \
    '  fi' \
    'done' \
    'exec "$REAL_GIT" "$@"' >"$fake_bin/git"
  chmod +x "$fake_bin/git"
  old_path=$PATH
  REAL_GIT=$real_git
  export REAL_GIT
  PATH="$fake_bin:$PATH"
  if scan "$fixture_repo" "$CAPS_FILE" >"$out" 2>&1; then
    echo "FAIL: partial Git enumeration returned success"
    failures=$((failures + 1))
  elif grep -qF "Git Rust-source enumeration failed" "$out"; then
    echo "PASS: partial Git enumeration fails closed"
  else
    echo "FAIL: partial Git enumeration lacked the fail-closed diagnostic:"
    cat "$out"
    failures=$((failures + 1))
  fi
  PATH=$old_path
  unset REAL_GIT

  # 6. Missing or malformed cap policy must fail closed instead of silently
  #    applying DEFAULT_CAP to every path.
  missing_caps="$fixture_repo/missing-caps.tsv"
  if scan "$fixture_repo" "$missing_caps" >"$out" 2>&1; then
    echo "FAIL: missing cap policy returned success"
    failures=$((failures + 1))
  elif grep -qF "caps policy is not readable" "$out"; then
    echo "PASS: missing cap policy fails closed"
  else
    echo "FAIL: missing cap policy lacked the fail-closed diagnostic:"
    cat "$out"
    failures=$((failures + 1))
  fi

  # 7. A Git repository with zero Rust sources must fail loudly.
  env GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
    git -c core.hooksPath=/dev/null -C "$empty_repo" init -q
  if scan "$empty_repo" "$CAPS_FILE" >"$out" 2>&1; then
    echo "FAIL: scanning a Git repository with zero Rust files returned success"
    failures=$((failures + 1))
  elif grep -qF "scanned 0 *.rs files under $empty_repo" "$out"; then
    echo "PASS: scanning zero Rust files fails loud"
  else
    echo "FAIL: zero-file failure lacked expected detail:"
    cat "$out"
    failures=$((failures + 1))
  fi

  # 8. A file whose final line has no trailing newline must still be
  #    counted correctly. `wc -l` undercounts such a file by one line
  #    (it counts newline characters, not lines); this gate must not.
  no_newline_file="$fixture_repo/src/no-trailing-newline.rs"
  awk -v n="$DEFAULT_CAP" 'BEGIN {
    for (i = 0; i < n; i++) printf "// selftest line\n"
    printf "// selftest final line without a trailing newline"
  }' >"$no_newline_file"
  expected_lines=$((DEFAULT_CAP + 1))
  if scan "$fixture_repo" "$CAPS_FILE" >"$out" 2>&1; then
    echo "FAIL: gate did not catch a file over cap whose final line lacks a trailing newline (wc -l would have undercounted it by one)"
    failures=$((failures + 1))
  elif grep -qF "src/no-trailing-newline.rs is $expected_lines lines (default cap $DEFAULT_CAP)" "$out"; then
    echo "PASS: a file whose final line lacks a trailing newline is counted correctly ($expected_lines lines, not undercounted)"
  else
    echo "FAIL: violation message missing expected file/size/cap detail for the no-trailing-newline file:"
    cat "$out"
    failures=$((failures + 1))
  fi
  rm -f -- "$no_newline_file"

  # 9. The gate resolves its root from its own location, not the caller's CWD.
  cwd_out=$(cd /tmp && "$SCRIPT_DIR/module-size-gate.sh") && cwd_status=0 || cwd_status=$?
  if [ "$cwd_status" -ne 0 ]; then
    echo "FAIL: invoking the gate from /tmp (unrelated CWD) did not exit 0:"
    echo "$cwd_out"
    failures=$((failures + 1))
  elif echo "$cwd_out" | grep -qE "^module-size gate ok: $baseline_count tracked or nonignored untracked \*\.rs files scanned"; then
    echo "PASS: unrelated CWD still resolves the canonical repo ($baseline_count files)"
  else
    echo "FAIL: unrelated-CWD invocation reported an unexpected source count:"
    echo "$cwd_out"
    failures=$((failures + 1))
  fi

  # 10. No self-test touched the real checkout; it must remain green and stable.
  if scan "$REPO_ROOT" "$CAPS_FILE" >"$out" 2>&1; then
    if [ "$SCAN_COUNT" -eq "$baseline_count" ]; then
      echo "PASS: real checkout stayed unchanged throughout self-test ($SCAN_COUNT files scanned)"
    else
      echo "FAIL: real checkout scan count changed from $baseline_count to $SCAN_COUNT"
      failures=$((failures + 1))
    fi
  else
    echo "FAIL: real checkout is not green after isolated self-tests:"
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
  echo "module-size gate ok: $SCAN_COUNT tracked or nonignored untracked *.rs files scanned in the canonical checkout; every file is within its recorded (or default $DEFAULT_CAP-line) cap."
  exit 0
fi
exit 1
