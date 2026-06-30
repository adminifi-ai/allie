# Allie Verification Runbook

This runbook is the cold operator path for verifying Allie without chat context.

## Setup

Install browser-worker dependencies once per checkout:

```sh
npm ci
npx playwright install chromium
```

In a consuming repository that does not vendor Allie's worker yet, install the
Rust binary and browser worker checkout together:

```sh
git clone --depth 1 https://github.com/adminifi-ai/allie .allie/tooling/allie
cargo install --path .allie/tooling/allie --locked
cd .allie/tooling/allie
npm ci
npx playwright install chromium
cd -
ALLIE_BROWSER_WORKER=.allie/tooling/allie/workers/browser/run.mjs \
  allie verify --manifest .allie/manifest.yml --out .allie/verify/latest
```

CI should archive the whole `.allie/verify/latest` directory, not just
`reporters/`, because `reporters/allie-report.html` links to sibling map,
evidence, WCAG report, and release artifacts.

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
npm run action:smoke
npm run auth:smoke
npm run visibility:smoke
npm run coverage:smoke
npm run consumer:smoke
npm run consumer-cwd:smoke
npm run agentic:smoke
npm run release:smoke
npm run autonomous:smoke
npm run size:smoke
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

`npm run action:smoke` proves manifest state setup actions execute before
evidence capture. It clicks open the workbench fixture menu, waits for the panel,
asserts the DOM artifact contains the revealed link, then runs a negative
control where a missing action selector records `state-step-failed` and blocks:

```text
.allie/runs/action-steps-smoke/evidence.json
.allie/runs/action-steps-smoke/report.html
.allie/runs/action-steps-smoke/worker-request.json
.allie/runs/action-steps-smoke/artifacts/dom-open-menu.html
.allie/runs/action-steps-smoke/artifacts/dom-typed-email.html
.allie/runs/action-steps-negative-smoke/evidence.json
```

`npm run auth:smoke` proves authenticated coverage through both `allie run` and
the composed `allie verify` pipeline. It logs into the local auth fixture with
env-sourced credentials, reaches the gated dashboard without writing the
credential value to artifacts, proves the storageState hatch reaches the same
route without writing the storageState path to artifacts, and runs negative
controls that block on `auth-lost` instead of treating the HTTP-200 login wall
as covered app content:

```text
.allie/runs/auth-smoke/evidence.json
.allie/runs/auth-smoke-storage-state/evidence.json
.allie/runs/auth-smoke-neg/evidence.json
.allie/verify/auth-smoke/run/evidence.json
.allie/verify/auth-smoke/reporters/allie-report.json
.allie/verify/auth-smoke-storage-state/run/evidence.json
.allie/verify/auth-smoke-storage-state/reporters/allie-report.json
.allie/verify/auth-smoke-neg/run/evidence.json
.allie/verify/auth-smoke-neg/reporters/allie-report.json
```

`npm run consumer:smoke` proves the portable consuming-app contract. It
scaffolds a manifest with `allie init`, runs `allie verify` over the same
manifest, checks that GitHub and Azure examples call the same CLI command, and
leaves:

```text
.allie/consumer-contract-smoke/manifest.yml
.allie/consumer-contract-smoke/discovery/discovery.json
.allie/consumer-contract-smoke/flow/generated-flow.yml
.allie/consumer-contract-smoke/map/product-map.json
.allie/consumer-contract-smoke/run/evidence.json
.allie/consumer-contract-smoke/report/compliance-report.json
.allie/consumer-contract-smoke/report/compliance-report.html
.allie/consumer-contract-smoke/release/release-summary.json
.allie/consumer-contract-smoke/reporters/allie-report.json
.allie/consumer-contract-smoke/reporters/allie-compliance-report.json
.allie/consumer-contract-smoke/reporters/allie-report.html
.allie/consumer-contract-smoke/reporters/allie-report.md
.allie/consumer-contract-smoke/reporters/junit.xml
.allie/consumer-contract-smoke/reporters/allie.sarif
```

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
.allie/jobs/autonomous-smoke/job.json
.allie/jobs/autonomous-smoke/events.jsonl
.allie/jobs/autonomous-smoke/steps/discovery/discovery.json
.allie/jobs/autonomous-smoke/steps/map/product-map.json
.allie/jobs/autonomous-smoke/steps/run/evidence.json
.allie/jobs/autonomous-smoke/steps/report/compliance-report.json
.allie/jobs/autonomous-smoke/steps/review/evidence-reviewed.json
.allie/jobs/autonomous-smoke/steps/release/release-summary.json
.allie/jobs/autonomous-agentic-smoke/steps/run/agentic-request.json
.allie/jobs/autonomous-agentic-smoke/steps/run/agentic-response.json
.allie/jobs/autonomous-agentic-smoke/steps/run/evidence.json
.allie/jobs/autonomous-agentic-smoke/steps/report/compliance-report.json
.allie/jobs/autonomous-agentic-smoke/steps/release/release-summary.json
.allie/discovery/autonomous-smoke/discovery.json
.allie/discovery/autonomous-smoke/flow-plan.json
.allie/discovery/autonomous-smoke/generated-flow.yml
.allie/runs/autonomous-smoke/evidence.json
.allie/reviews/autonomous-smoke/evidence-reviewed.json
.allie/releases/autonomous-smoke/release-summary.json
```

The smoke expects the generated replay to find the fixture's known deterministic
contrast issue. That failure proves enforcement is wired: review still runs, and
release projection blocks on deterministic evidence while keeping agentic context
non-authoritative.

Use the job status command to inspect the durable run state:

```sh
cargo run --locked -- workbench status --job .allie/jobs/autonomous-smoke
```

The expected completed fixture job has `schema: allie.job.v0`, `status:
blocked`, release status `blocked`, resumable state, and artifact pointers for
map, evidence, compliance report, review, and release outputs. The smoke also
verifies that `workbench start` refuses to reuse an existing job directory,
that non-local advisory agent modes stay on the one-shot `map` path until
durable session adapters exist, and that a `model.enabled` workbench job runs
the live agentic gateway before report/release. The model-enabled smoke unsets
the configured API key, so the gateway must still capture media and write
degraded `inconclusive` assessments without fabricating pass/fail verdicts.

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

For `allie verify`, `approved` and `needs_review` exit `0`; `blocked` exits `1`
because required evidence blocks the projection; infrastructure failures exit
`2`.

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
