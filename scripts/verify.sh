#!/bin/sh
set -eu

cargo fmt --check
cargo test --locked
npm run worker:smoke
npm run evidence:smoke
npm run visibility:smoke
npm run release:smoke
npm run autonomous:smoke

test -f .allie/runs/v0-smoke/evidence.json
test -f .allie/runs/v0-smoke/report.html
test -f .allie/maps/v0-smoke/product-map.json
test -f .allie/maps/v0-smoke/surface-map.html
test -f .allie/maps/v0-smoke/agent-runner-receipt.json
test -f .allie/maps/v0-smoke/generated-flow.yml
test -f .allie/reports/v0-smoke/compliance-report.json
test -f .allie/reports/v0-smoke/compliance-report.html
test -f .allie/reports/v0-smoke/summary.md
test -f .allie/releases/v0-smoke/release-summary.json
test -f .allie/releases/v0-smoke/github-check.json
test -f .allie/releases/v0-smoke/release-report.html
test -f .allie/discovery/autonomous-smoke/discovery.json
test -f .allie/discovery/autonomous-smoke/flow-plan.json
test -f .allie/runs/autonomous-smoke/evidence.json
test -f .allie/reviews/autonomous-smoke/evidence-reviewed.json
test -f .allie/remediation/autonomous-smoke/remediation-queue.json
test -f .allie/remediation/autonomous-smoke/patch-plan.md
test -f .allie/releases/autonomous-smoke/release-summary.json
test -f .allie/jobs/autonomous-smoke/job.json
test -f .allie/jobs/autonomous-smoke/events.jsonl
test -f .allie/jobs/autonomous-smoke/steps/map/product-map.json
test -f .allie/jobs/autonomous-smoke/steps/report/compliance-report.json
