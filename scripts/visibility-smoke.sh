#!/bin/sh
set -eu

cargo run --locked -- map \
  --manifest examples/login-flow.yml \
  --project-root . \
  --out .allie/maps/v0-smoke

cargo run --locked -- report \
  --map .allie/maps/v0-smoke/product-map.json \
  --packet .allie/runs/v0-smoke/evidence.json \
  --out .allie/reports/v0-smoke

test -f .allie/maps/v0-smoke/product-map.json
test -f .allie/maps/v0-smoke/surface-map.html
test -f .allie/maps/v0-smoke/agent-runner-receipt.json
test -f .allie/maps/v0-smoke/generated-flow.yml
test -f .allie/reports/v0-smoke/compliance-report.json
test -f .allie/reports/v0-smoke/compliance-report.html
test -f .allie/reports/v0-smoke/summary.md
