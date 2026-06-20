# Allie Verification Runbook

This runbook is the cold operator path for verifying Allie without chat context.

## Setup

Install browser-worker dependencies once per checkout:

```sh
npm ci
npx playwright install chromium
```

## Gate

Run the full local gate:

```sh
npm run verify
```

Equivalent expanded commands:

```sh
cargo fmt --check
cargo test --locked
npm run worker:smoke
npm run evidence:smoke
npm run release:smoke
npm run autonomous:smoke
```

## Expected Evidence

`npm run worker:smoke` proves the Node worker can run Playwright plus axe. It
leaves:

```text
.allie/worker-smoke/worker-request.json
.allie/worker-smoke/worker-response.json
.allie/worker-smoke/artifacts/axe-login-form.json
.allie/worker-smoke/artifacts/login-form.png
```

`npm run evidence:smoke` proves the Rust CLI, worker, packet writer, and report
writer work together. It leaves:

```text
.allie/runs/v0-smoke/evidence.json
.allie/runs/v0-smoke/report.html
.allie/runs/v0-smoke/artifacts/axe-login-form.json
.allie/runs/v0-smoke/artifacts/login-form.png
```

The expected happy-path packet summary is `status: pass`, `exit_code: 0`,
one captured state, and artifact types `axe_json`, `screenshot`, and
`html_report`.

`npm run release:smoke` proves the packet can drive release decisions without a
second status model. It reads `.allie/runs/v0-smoke/evidence.json` and leaves:

```text
.allie/releases/v0-smoke/release-summary.json
.allie/releases/v0-smoke/github-check.json
.allie/releases/v0-smoke/release-report.html
```

The expected V0 fixture decision is `needs_review` with a neutral GitHub check:
deterministic evidence passed, but keyboard/focus/zoom/reduced-motion and human
assistive-technology obligations are still marked `not_tested` or
`needs_review`.

`npm run autonomous:smoke` proves the autonomous workbench path. It leaves:

```text
.allie/discovery/autonomous-smoke/discovery.json
.allie/discovery/autonomous-smoke/flow-plan.json
.allie/discovery/autonomous-smoke/generated-flow.yml
.allie/runs/autonomous-smoke/evidence.json
.allie/reviews/autonomous-smoke/evidence-reviewed.json
.allie/remediation/autonomous-smoke/remediation-queue.json
.allie/releases/autonomous-smoke/release-summary.json
```

The smoke expects the generated replay to find the fixture's known deterministic
contrast issue. That failure proves enforcement and remediation are wired:
review still runs, remediation writes a queue, and release projection blocks on
deterministic evidence while keeping agentic context non-authoritative.

## Failure Meanings

- Exit `0`: evidence passed for the required deterministic/scripted checks.
- Exit `1`: deterministic or scripted required evidence failed, such as an axe
  violation, HTTP route-state failure, or missing required artifact.
- Exit `2`: infrastructure or trust-boundary failure, such as missing
  credentials, incomplete model policy, worker crash, timeout, unreachable
  target, partial worker response, or nondeterminism.
- Exit `64`: CLI usage error.

For `allie release`, exit `1` means a packet failure, missing changed-surface
evidence, expired touched waiver, or invalid touched-waiver metadata blocked the
release decision. Stale evidence and model-only findings are review-required
neutral outputs, not hard release blocks.

## Trust-Boundary Fixtures

These are negative verification fixtures:

```sh
cargo run --locked -- run --manifest examples/trust-missing-credential.yml --out .allie/runs/trust-boundary-missing-credential
cargo run --locked -- run --manifest examples/trust-model-policy-incomplete.yml --out .allie/runs/trust-boundary-model-policy
cargo run --locked -- run --manifest examples/trust-unreachable-target.yml --out .allie/runs/trust-boundary-unreachable
```

Each should exit `2` and write an `error` packet explaining the failure class.

## Release Projection

Project any packet into release/check outputs:

```sh
cargo run --locked -- release --packet .allie/runs/v0-smoke/evidence.json --out .allie/releases/v0-smoke --changed-surface login-form
```

Use one `--changed-surface <id>` per touched surface. The release projection
matches each changed surface against packet `coverage.states_captured` and
`coverage.surfaces_discovered`.

## Cleanup

Generated receipts live under `.allie/` and are ignored by Git. Remove them
when they are no longer needed:

```sh
rm -rf .allie
```
