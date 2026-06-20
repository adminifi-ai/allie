# Freeze V0 around a deterministic local evidence loop

Priority: P0 - Status: done - Estimate: XL

## PRD Summary

- User: frontend engineers and QA engineers proving one critical staged-app flow
  before release.
- Problem: the repo has strong product prose, but no executable acceptance path
  for the V0 command in `SPEC.md`.
- Goal: make `allie run --manifest examples/login-flow.yml --out .allie/runs/latest`
  drive one checked-in local flow and emit replayable evidence.
- Why now: every later feature depends on a stable manifest in, evidence packet
  out loop.
- UX enabled: a cold operator can run one command, inspect a local HTML report,
  and see deterministic pass/fail behavior with replay instructions.
- Deliverable type: working code.
- Success signal: the V0 command produces the required artifact set and
  deterministic exit behavior under `cargo test --locked`.

## Product Requirements

- P0: parse and validate a flow manifest for `examples/login-flow.yml`.
- P0: run a checked-in local fixture through one Playwright-driven route state.
- P0: run axe on the inspected state and preserve raw axe JSON.
- P0: write an `allie.evidence.v0` packet plus a local HTML report under the
  requested output directory.
- P0: return a blocking exit code only for deterministic or scripted required
  failures.
- P0: document the replay command inside the evidence packet and report.
- P1: include one screenshot and minimal state metadata.
- Non-goals: real staging credentials, model review, PR comments, hosted
  dashboards, remediation branches, video/GIF, full DOM capture, and legal
  compliance claims.

## Technical Design

- Chosen architecture: Rust CLI orchestrator owns manifest parsing, run
  planning, evidence writing, report generation, and exit taxonomy; a narrow
  Node worker owns Playwright and axe execution.
- Files/systems touched: `src/`, `Cargo.toml`, `Cargo.lock`, `examples/`,
  `fixtures/`, worker package files, evidence/report templates, and repo tests.
- Data/control flow: CLI validates manifest -> plans one run -> invokes worker
  with typed request -> receives route-state and axe artifact metadata -> writes
  packet/report -> returns deterministic exit code.
- Build/check boundary: Rust compilation and unit/contract tests catch schema,
  parser, writer, and exit behavior; worker smoke catches browser/axe runtime.
- ADR decision: not required for the first slice; escalate to ADR if the worker
  protocol becomes multi-process orchestration rather than one request/response
  boundary.
- ADR-style invariants: Playwright details stay behind the worker adapter
  (`docs/architecture.md`); the evidence packet remains the core interface
  (`AGENTS.md`); no model-only finding blocks release (`SPEC.md`).
- Design X vs Y: choose a local fixture with optional/no-op auth before real
  staging auth; choose one screenshot and axe JSON before rich artifact capture;
  choose a typed worker protocol before embedding Node details in Rust.

## Lead Repo Read

- `SPEC.md`: V0 acceptance command, evidence packet requirements, gate policy,
  security contract.
- `README.md`: current CLI placeholder and first real CLI milestone.
- `docs/architecture.md`: Rust core, worker adapter, evidence store, and deep
  narrow interface constraints.
- `docs/evidence-contract.md`: current packet draft and replay fields.
- `docs/roadmap.md`: first acceptance slice and required evidence.
- `src/main.rs`, `Cargo.toml`, `AGENTS.md`.
- Commands: `cargo fmt --check`, `cargo test --locked`, `git ls-files`,
  `find .github ...`, `rg TODO|FIXME|HACK`.

## Alignment Questions

- Should V0 require real staging credentials? Recommended answer: no; start
  with a checked-in fixture and no-op auth profile, then add real credentials
  after replay is stable. Risk if wrong: the first loop spends effort on auth
  plumbing before proving evidence semantics.
- Should V0 include model findings? Recommended answer: no; schema can reserve
  disabled audit fields, but no provider calls run. Risk if wrong: model policy
  leaks into the deterministic wedge.
- Should DOM and accessibility-tree snapshots ship in V0? Recommended answer:
  no; keep axe JSON, one screenshot, packet, report, and replay command first.
  Risk if wrong: privacy/redaction scope blocks the core loop.
- Should the first report optimize for compliance stakeholders? Recommended
  answer: no; optimize for developer/QA replay and explicit residual review
  paths. Risk if wrong: report polish outruns proof.

## Deliverable

- Output: working V0 CLI command, fixture, manifest, worker contract, evidence
  packet, local report, tests, and smoke command.
- Acceptance oracle: `cargo run --locked -- run --manifest examples/login-flow.yml --out .allie/runs/latest`
  exits according to deterministic results and leaves `evidence.json`,
  `report.html`, axe JSON, one screenshot, and replay instructions.
- Evidence artifacts: `.allie/runs/latest/evidence.json`,
  `.allie/runs/latest/report.html`, worker stdout/stderr summary, test output.
- Residual risk: real staging auth, rich redaction, model review, PR checks, and
  human review workflow remain unproven.

## Goal

Make the V0 acceptance command produce a replayable local accessibility evidence
packet and report for one checked-in fixture flow.

## Oracle

- [ ] `cargo run --locked -- run --manifest examples/login-flow.yml --out .allie/runs/latest`
  produces the expected artifact set.
- [ ] `cargo test --locked` covers manifest validation, packet writing, report
  generation, and deterministic exit semantics.
- [ ] The worker smoke runs Playwright plus axe against the checked-in fixture.
- [ ] `AGENTS.md` names the worker smoke once the real browser worker exists.

## Verification System

- Claim: one command proves the local deterministic evidence loop works.
- Falsifier: missing artifact, invalid packet, worker failure, unmapped axe
  result, or wrong exit code.
- Driver: the V0 command against `examples/login-flow.yml`, plus `cargo test --locked`.
- Grader: schema validation, artifact existence and hashes, expected exit code,
  and report links to replay instructions.
- Evidence packet: `.allie/runs/latest/` plus test output.
- Cadence: before implementation, after each child milestone, and before merge.

## Children

1. Define the formal `allie.evidence.v0` JSON Schema and Rust packet types.
2. Define the flow manifest schema and check in `examples/login-flow.yml`.
3. Add the smallest local fixture app that can exercise one meaningful route.
4. Implement the `allie run` CLI surface with validation and output directory
   handling.
5. Define the typed worker request/response protocol and a minimal Playwright
   plus axe worker.
6. Write the evidence packet, local report, screenshot, axe JSON, and replay
   instructions.
7. Add deterministic exit semantics and update repo gates with the worker smoke.

## Notes

**Why:** Product/vision, architecture, verification, simplification, and
external exemplar lanes all converge on making the evidence loop executable
before expanding the product surface. Playwright's official accessibility
testing guidance points to Axe integration, and axe-core exposes WCAG-tagged
rule metadata suitable for initial deterministic mapping.

## Delivered

- Implemented `cargo run --locked -- run --manifest examples/login-flow.yml --out .allie/runs/latest`.
- Added a checked-in login fixture, flow manifest, Playwright/axe worker, raw axe JSON artifact, screenshot artifact, `allie.evidence.v0` packet, local HTML report, replay command, and deterministic/scripted exit semantics.
- Added fail-closed checks for required route HTTP failures and missing required worker artifacts.
- Verified with `cargo fmt --check`, `cargo test --locked`, `npm run worker:smoke`, the V0 command, and a negative `/missing` route CLI run.
