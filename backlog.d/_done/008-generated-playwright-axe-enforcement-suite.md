# Generate comprehensive Playwright and axe enforcement coverage

Priority: P1 - Status: done - Estimate: XL

## PRD Summary

- User: engineers and QA teams who need accessibility regressions caught in CI.
- Problem: Allie runs axe on one captured state today; it does not generate or
  enforce broad Playwright coverage across discovered flows and interaction
  states.
- Goal: generate comprehensive browser tests from verified discoveries and bind
  those tests to WCAG obligations and release policy.
- Why now: once discovery and WCAG drilldown exist, Allie needs executable tests
  that keep those obligations current.
- UX enabled: teams receive generated tests for critical flows instead of a
  one-off scanner report.
- Deliverable type: generated tests, worker capabilities, and release gate.
- Success signal: generated tests replay against fixture and staged targets,
  preserve evidence artifacts, and block only deterministic/scripted failures.

## Product Requirements

- P0: generate Playwright flows for verified discovered surfaces, including
  keyboard traversal, focus order, modal/menu behavior, form error recovery,
  viewport/zoom/reflow, reduced motion, and visible dynamic states.
- P0: run axe after route and interaction states, including content hidden behind
  activated menus/dialogs when relevant.
- P0: capture DOM, accessibility tree, screenshot, video/GIF or trace,
  console/network summaries, and raw axe JSON under redaction policy.
- P0: map each generated assertion to WCAG obligation methods and report drilldown
  entries.
- P0: mark generated tests as release-required only after replay success.
- P1: emit an optional checked-in Playwright test bundle for consumer repos.
- Non-goals: bypassing application auth policy, weakening deterministic gates, or
  blocking releases on model-only findings.

## Technical Design

- Rust owns generated-test metadata, promotion state, release policy, and packet
  output.
- Worker owns Playwright execution and axe integration behind typed contracts.
- Test generation consumes `verified_flow` records, not raw crawl hypotheses.
- Generated test artifacts remain reproducible through replay commands and hashes.

## Lead Repo Read

- `workers/browser/run.mjs`: current browser/axe execution boundary.
- `src/lib.rs`: current flow state model, artifact model, and release projection.
- `profiles/wcag22-aa.json`: current scripted obligations are placeholders.
- `docs/verification.md`: existing smoke gates and evidence receipts.

## Deliverable

- Output: generated test contract, worker support for scripted interactions,
  artifact capture expansion, and release enforcement for verified generated
  tests.
- Acceptance oracle: generated tests from the fixture corpus replay in CI and
  produce criterion-linked evidence, with known failures blocking and known gaps
  reported as `not_tested` or `needs_review`.
- Evidence artifacts: generated Playwright bundle, worker transcript, raw axe
  JSON, screenshots, DOM/a11y-tree snapshots, videos/GIFs/traces, and HTML
  drilldown report.
- Residual risk: generated coverage still requires periodic human review for
  business-critical journeys Allie cannot infer.

## Verification System

- Claim: generated browser coverage is enforceable only after replay.
- Falsifier: generated assertion has no WCAG mapping; hidden state is never
  activated; unreplayed generated test blocks release; artifact capture leaks
  unredacted sensitive data; or CI passes while known fixture failures disappear.
- Driver: fixture corpus with known good/bad scripted states.
- Grader: replay output, failing-fixture assertions, report links, release
  projection, and redaction checks.
- Evidence packet: generated-test replay packets plus release summaries.
- Cadence: after discovery promotion and WCAG ledger are available.

## Children

1. Extend worker protocol for interaction scripts and state capture.
2. Generate Playwright flows from verified flow plans.
3. Add axe-after-interaction and hidden-state activation coverage.
4. Capture DOM/a11y tree, video/GIF/trace, console, and network artifacts.
5. Map generated assertions to WCAG methods.
6. Add release policy for replayed generated tests.

## Notes

Deque documents axe rule tags for WCAG versions and success criteria, and notes
that hidden regions need to be activated or rendered before analysis. Generated
Playwright coverage is how Allie should make that requirement practical.

## Delivered

- Promoted discovered surfaces into generated replay manifests with axe,
  screenshot, DOM snapshot, accessibility-tree, keyboard, video request, and
  trace flags.
- Expanded the browser worker to capture DOM snapshots, DOM-derived
  accessibility-tree JSON, keyboard focus order, trace JSON, screenshots, raw axe
  JSON, and opportunistic stable video artifacts.
- Generated replay remains the enforcement boundary: the autonomous smoke expects
  the known fixture contrast issue to fail and block release.
- Verified by `npm run autonomous:smoke`, including artifact-type assertions for
  `axe_json`, `screenshot`, `dom_snapshot`, `accessibility_tree`, `trace`, and
  `html_report`.
