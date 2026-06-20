# Allie

Allie is a Rust-first accessibility evidence harness and release intelligence system.

The product goal is not to be another accessibility scanner. Allie should map staged web applications and critical user flows, run deterministic accessibility checks, capture replayable evidence, enrich judgment-heavy criteria with multimodal agents, and make accessibility status visible in pull requests and release decisions.

Positioning:

> Accessibility evidence for every release.

## Why This Exists

Accessibility work is often split across manual expert review, browser extensions, point-in-time axe runs, ad hoc screenshots, and release conversations that are hard to reproduce. Allie turns that into an evidence system:

- deterministic checks where machines can be certain;
- scripted browser flows where interaction behavior matters;
- screenshots, video, DOM, and accessibility tree artifacts where human or agent review needs context;
- standards-mapped status across WCAG, ADA, Section 508, and client policy packs;
- PR and release gates that block real regressions without pretending uncertain findings are certain.

## Initial Shape

- Rust CLI and orchestrator.
- Node/Playwright/axe worker boundary for browser automation.
- Evidence packets as the durable contract.
- Model gateway for multimodal first-pass review, with OpenRouter only behind strict privacy and provider-routing policy.
- Local HTML/JSON reports first; hosted dashboards later.

## Repository Map

- [SPEC.md](SPEC.md): product contract and acceptance model.
- [docs/architecture.md](docs/architecture.md): proposed system design.
- [docs/evidence-contract.md](docs/evidence-contract.md): first evidence schema shape.
- [docs/naming.md](docs/naming.md): naming decisions and alternates.
- [docs/roadmap.md](docs/roadmap.md): proposed build sequence.

## V0 Local Evidence Loop

```sh
cargo run --locked -- run --manifest examples/login-flow.yml --out .allie/runs/latest
```

The V0 command runs the checked-in login fixture through the browser worker,
captures axe output and a screenshot, and writes a replayable evidence packet
plus a local HTML report:

```sh
.allie/runs/latest/evidence.json
.allie/runs/latest/report.html
.allie/runs/latest/artifacts/axe-login-form.json
.allie/runs/latest/artifacts/login-form.png
```

The packet reports accessibility evidence status, confidence, and residual
review needs. It is not a legal compliance guarantee.

## Release Decision Projection

```sh
cargo run --locked -- release --packet .allie/runs/latest/evidence.json --out .allie/releases/latest --changed-surface login-form
```

The release command reads an `allie.evidence.v0` packet and writes a release
summary, a GitHub Checks-style payload, and an HTML decision report:

```sh
.allie/releases/latest/release-summary.json
.allie/releases/latest/github-check.json
.allie/releases/latest/release-report.html
```

It blocks on packet failures, missing evidence for changed surfaces, expired
waivers on touched surfaces, and invalid waiver metadata. Stale evidence,
model-only findings, `needs_review` obligations, and `not_tested` obligations
produce a neutral review-required decision instead of a hard block.

## Autonomous Workbench Loop

```sh
cargo run --locked -- workbench start --manifest examples/autonomous-workbench.yml --out .allie/jobs/autonomous
```

The workbench command is the durable operator entrypoint for autonomous work. It
writes a job ledger, lifecycle events, step receipts, generated artifacts, and
final pointers under one directory:

```sh
.allie/jobs/autonomous/job.json
.allie/jobs/autonomous/events.jsonl
.allie/jobs/autonomous/steps/discovery/discovery.json
.allie/jobs/autonomous/steps/map/product-map.json
.allie/jobs/autonomous/steps/run/evidence.json
.allie/jobs/autonomous/steps/report/compliance-report.json
.allie/jobs/autonomous/steps/review/evidence-reviewed.json
.allie/jobs/autonomous/steps/release/release-summary.json
```

Inspect, cancel, or resume the job with:

```sh
cargo run --locked -- workbench status --job .allie/jobs/autonomous
cargo run --locked -- workbench cancel --job .allie/jobs/autonomous
cargo run --locked -- workbench resume --job .allie/jobs/autonomous
```

The one-shot task primitives remain available for debugging or custom
orchestration:

```sh
cargo run --locked -- discover --manifest examples/autonomous-workbench.yml --out .allie/discovery/autonomous
cargo run --locked -- promote-flow --discovery .allie/discovery/autonomous/discovery.json --flow-plan .allie/discovery/autonomous/flow-plan.json --out .allie/discovery/autonomous/generated-flow.yml
cargo run --locked -- run --manifest .allie/discovery/autonomous/generated-flow.yml --out .allie/runs/autonomous
cargo run --locked -- review --packet .allie/runs/autonomous/evidence.json --out .allie/reviews/autonomous
cargo run --locked -- remediate --packet .allie/runs/autonomous/evidence.json --out .allie/remediation/autonomous
```

The autonomous loop discovers fixture surfaces, promotes generated flow
candidates into a replayable manifest, captures axe, screenshot, DOM,
accessibility-tree, keyboard, and trace artifacts, adds offline agentic review
context with redaction receipts, and writes an evidence-linked remediation
queue. Generated and agentic claims do not enforce release policy until replayed,
scripted, or human-attested.

## Local Verification

Install the browser worker dependencies once:

```sh
npm install
npx playwright install chromium
```

Then run the repo gates:

```sh
cargo fmt --check
cargo test --locked
npm run worker:smoke
npm run evidence:smoke
npm run release:smoke
npm run autonomous:smoke
cargo run --locked -- run --manifest examples/login-flow.yml --out .allie/runs/latest
cargo run --locked -- release --packet .allie/runs/latest/evidence.json --out .allie/releases/latest --changed-surface login-form
```

The worker smoke proves Playwright plus axe can inspect the checked-in fixture.
The evidence smoke leaves a stable receipt under `.allie/runs/v0-smoke/`. The
release smoke projects that packet into `.allie/releases/v0-smoke/`. The final
two commands are the V0 live oracle and release projection, leaving inspectable
evidence under `.allie/runs/latest/` and `.allie/releases/latest/`.
The autonomous smoke leaves discovery, generated-flow, richer evidence, review,
remediation, and blocked-release receipts under `.allie/*/autonomous-smoke/`.
It also leaves durable workbench lifecycle receipts under
`.allie/jobs/autonomous-smoke/`.

For a cold-start verification path, see [docs/verification.md](docs/verification.md).
