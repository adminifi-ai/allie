#!/bin/sh
# Module size gate (ratchet).
#
# The lib.rs decomposition brought the largest Rust source file from 10055 lines
# down to ~7980, extracting src/report.rs (rendering), src/agentic.rs (agentic
# review), and src/compliance.rs (compliance model). This gate locks that win:
# no file under src/ may exceed CAP lines.
#
# If a file trips this, EXTRACT a cohesive module (see docs/roadmap.md "Code
# Health Backlog") rather than raising CAP. Lower CAP as the files shrink further
# — it should ratchet down, never up, absent a deliberate, reviewed decision.
set -eu

CAP=8200
status=0

for f in src/*.rs; do
  n=$(wc -l < "$f")
  if [ "$n" -gt "$CAP" ]; then
    echo "FAIL: $f is $n lines (cap $CAP). Extract a cohesive module instead of growing it."
    status=1
  fi
done

if [ "$status" -eq 0 ]; then
  echo "module-size gate ok: every src/*.rs is <= $CAP lines (lib.rs was 10055 pre-decomposition)."
fi
exit "$status"
