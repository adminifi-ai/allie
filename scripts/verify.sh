#!/bin/sh
set -eu

cargo fmt --check
cargo test --locked
cargo clippy --locked -- -D warnings
npm run secrets:smoke
npm run worker:smoke
npm run evidence:smoke
npm run axe-rules:smoke
npm run action:smoke
npm run auth:smoke
npm run visibility:smoke
npm run coverage:smoke
npm run consumer:smoke
npm run consumer-cwd:smoke
npm run distribution:smoke
npm run agentic:smoke
npm run agentic:precision
npm run release:smoke
npm run autonomous:smoke
npm run size:smoke

test -f .allie/runs/v0-smoke/evidence.json
test -f .allie/runs/v0-smoke/report.html
test -f .allie/runs/action-steps-smoke/evidence.json
test -f .allie/runs/action-steps-smoke/artifacts/dom-open-menu.html
test -f .allie/runs/action-steps-smoke/artifacts/dom-typed-email.html
test -f .allie/runs/action-steps-negative-smoke/evidence.json
test -f .allie/maps/v0-smoke/product-map.json
test -f .allie/maps/v0-smoke/surface-map.html
test -f .allie/maps/v0-smoke/agent-runner-receipt.json
test -f .allie/maps/v0-smoke/generated-flow.yml
test -f .allie/reports/v0-smoke/compliance-report.json
test -f .allie/reports/v0-smoke/compliance-report.html
test -f .allie/reports/v0-smoke/summary.md
test -f .allie/runs/coverage-matrix-smoke/evidence.json
test -f .allie/reports/coverage-matrix-smoke/compliance-report.json
test -f .allie/reports/coverage-matrix-smoke/compliance-report.html
test -f .allie/reports/coverage-matrix-smoke/summary.md
test -f .allie/consumer-contract-smoke/reporters/allie-report.json
test -f .allie/consumer-contract-smoke/reporters/allie-report.html
test -f .allie/consumer-contract-smoke/reporters/allie-report.md
test -f .allie/consumer-contract-smoke/reporters/junit.xml
test -f .allie/consumer-contract-smoke/reporters/allie.sarif
test -f .allie/distribution-smoke/consumer/.allie/run/latest/evidence.json
test -f .allie/distribution-smoke/consumer/.allie/doctor/doctor.json
test -f .allie/distribution-smoke/consumer/.allie/doctor/browser-worker-smoke/worker-response.json
test -f .allie/releases/v0-smoke/release-summary.json
test -f .allie/releases/v0-smoke/github-check.json
test -f .allie/releases/v0-smoke/release-report.html
test -f .allie/discovery/autonomous-smoke/discovery.json
test -f .allie/discovery/autonomous-smoke/flow-plan.json
test -f .allie/runs/autonomous-smoke/evidence.json
test -f .allie/reviews/autonomous-smoke/evidence-reviewed.json
test ! -d .allie/remediation/autonomous-smoke
test -f .allie/releases/autonomous-smoke/release-summary.json
test -f .allie/jobs/autonomous-smoke/job.json
test -f .allie/jobs/autonomous-smoke/events.jsonl
test -f .allie/jobs/autonomous-smoke/steps/map/product-map.json
test -f .allie/jobs/autonomous-smoke/steps/report/compliance-report.json
test -f .allie/jobs/autonomous-agentic-smoke/steps/run/agentic-request.json
test -f .allie/jobs/autonomous-agentic-smoke/steps/run/agentic-response.json
test -f .allie/jobs/autonomous-agentic-smoke/steps/run/evidence.json
test -f .allie/jobs/autonomous-agentic-smoke/steps/report/compliance-report.json
test -f .allie/jobs/autonomous-agentic-smoke/steps/release/release-summary.json
test -f .allie/jobs/autonomous-agentic-error-smoke/job.json
test -f .allie/jobs/autonomous-agentic-error-smoke/events.jsonl
