# Make autonomous discovery a versioned, replay-gated contract

Priority: P0 - Status: done - Estimate: XL

## PRD Summary

- User: accessibility compliance engineers starting an assessment against a real
  application.
- Problem: Allie currently requires a handwritten flow manifest and shallow
  changed-surface input, so it cannot autonomously identify the sitemap, product
  surfaces, or user stories that should be assessed.
- Goal: make `allie discover` emit a versioned discovery packet and promote
  selected discoveries into replayable generated flow candidates.
- Why now: every comprehensive WCAG, Playwright, agentic, remediation, and
  enforcement feature depends on trustworthy surface discovery.
- UX enabled: an operator points Allie at a staged app and receives an inspectable
  surface graph plus generated flow candidates before manually authoring tests.
- Deliverable type: working code and fixture corpus.
- Success signal: generated flows are not trusted until they replay through the
  existing evidence path.

## Product Requirements

- P0: discover route graph, page states, interactive regions, forms, dialogs,
  menus, media, authentication walls, and error states from a staged or fixture
  app within explicit route/time/budget limits.
- P0: generate a sitemap, product-surface inventory, and likely user stories with
  confidence and provenance for each item.
- P0: emit `allie.discovery.v0` and `allie.flow-plan.v0` packets with hashes,
  source URLs, discovery method, screenshots, DOM/a11y-tree refs when available,
  and replay metadata.
- P0: add a promotion state model: `generated_candidate`, `verified_flow`,
  `release_required`.
- P0: only `verified_flow` or explicitly pinned flows can feed release-required
  enforcement.
- P1: support operator review of discovered surfaces without requiring edits to
  the original flow manifest.
- Non-goals: remediation PRs, live model provider calls, hosted dashboard, or
  legal conformance claims.

## Technical Design

- Rust owns packet schemas, budgets, promotion state, hashing, and CLI commands.
- Browser worker owns crawling and browser state extraction behind typed
  request/response contracts.
- Generated flow candidates compile to the existing manifest/evidence pipeline
  before enforcement.
- Fixtures must include multi-page navigation, auth wall, modal/menu, form
  validation errors, keyboard trap, dynamic route, unreachable route, and flaky
  state.

## Lead Repo Read

- `examples/login-flow.yml`: current handwritten flow manifest.
- `workers/browser/run.mjs`: current worker only visits specified states.
- `src/lib.rs`: release projection currently depends on manually supplied changed
  surfaces.
- `schemas/allie.evidence.v0.schema.json`: formal schema is too loose for
  generated systems.
- `docs/architecture.md`: Rust core and worker boundaries.

## Deliverable

- Output: discovery CLI, discovery packet schema, generated flow-plan packet,
  fixture corpus, promotion command, and replay verification gate.
- Acceptance oracle: `allie discover --target <fixture> --out .allie/discovery/latest`
  emits discovery and flow-plan packets; `allie promote-flow ...` creates a
  generated manifest; the generated manifest replays through `allie run`.
- Evidence artifacts: `.allie/discovery/latest/discovery.json`,
  `.allie/discovery/latest/flow-plan.json`, generated manifest, replayed
  evidence packet, report, screenshots, and worker transcript.
- Residual risk: discovered user stories are hypotheses until verified by replay
  or operator attestation.

## Verification System

- Claim: autonomous discovery can generate candidate coverage without smuggling
  unverified agent output into release enforcement.
- Falsifier: generated flow enforces release policy before replay; missing route
  provenance; unbounded crawl; discovery packet cannot be replayed; or generated
  manifest breaks the existing evidence command.
- Driver: multi-page fixture discovery plus generated-flow replay.
- Grader: schema validation, promotion-state assertions, artifact links, replay
  success, and release projection tests.
- Evidence packet: discovery packet, flow-plan packet, replay evidence packet,
  and generated report.
- Cadence: before implementation, after crawler boundary, after promotion, and
  before merge.

## Children

1. Define `allie.discovery.v0` and `allie.flow-plan.v0` schemas.
2. Add multi-page fixture corpus with known accessible and inaccessible states.
3. Add worker discovery request/response protocol with route/time/artifact
   budgets.
4. Implement `allie discover` and surface/user-story generation from browser
   evidence.
5. Implement promotion from generated candidate to replayed manifest.
6. Add release projection tests that reject unverified generated flows.

## Notes

W3C WCAG-EM frames evaluation around scope, website/app exploration, sample
selection, evaluation, and reporting. Allie should operationalize that shape
without pretending discovery is equivalent to conformance.

## Delivered

- Added `allie discover` and `allie promote-flow`.
- Added `allie.discovery.v0` and `allie.flow-plan.v0` packet generation.
- Added the multi-page `examples/autonomous-workbench.yml` fixture and
  `fixtures/workbench/` surface set.
- Generated manifests now normalize fixture paths and mark promoted states as
  `verified_flow`.
- Verified by `npm run autonomous:smoke`, which emits discovery/flow-plan
  packets, promotes a generated manifest, and replays it through `allie run`.
