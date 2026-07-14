# Roadmap

## Current V0 Loop

The first local evidence loop is implemented around the portable consumer
contract:

```sh
allie init --manifest .allie/manifest.yml --app-name "My App"
allie doctor --manifest .allie/manifest.yml --out .allie/doctor
allie verify --manifest .allie/manifest.yml --out .allie/verify/latest
```

`allie verify` composes discovery, generated-flow promotion, product mapping,
browser evidence capture, WCAG reporting, and release projection into one
host-agnostic command. It writes stable JSON, HTML, Markdown, JUnit, and SARIF
reporters under `reporters/`.

This is a proof foundation, not a legal compliance guarantee. The current
system discovers static/local surfaces, generates replay manifests, runs
deterministic browser evidence, builds a complete WCAG 2.2 A/AA matrix, adds
offline agentic review context, and projects release decisions. Authenticated
staged-app crawling and live multimodal provider calls remain future work.

## Product Target

Allie should let a release-owning engineer dispatch it against a repository and
let an accessibility engineer investigate the resulting report without either
having to hand-author the product map. They receive:

1. An autonomously discovered sitemap, product-surface inventory, and likely user
   stories.
2. A compact, durable accessibility test plan whose generated static, unit,
   integration, Playwright, axe, and agentic checks run ephemerally in an
   isolated workspace before they can enforce release policy.
3. A complete WCAG 2.2 A/AA obligation ledger with drilldown from criterion to
   state, finding, artifact, agentic context, waiver, and replay proof.
4. Agentic vision review for criteria that require judgment, with redaction
   receipts and neutral findings until promoted by scripted proof or human
   attestation.
5. Release enforcement that is a packet projection, not a separate status model.

## Now

1. Keep the autonomous workbench and consumer contract smokes green as the
   primary regression oracles (`npm run autonomous:smoke` and
   `npm run consumer:smoke`).
2. Dogfood `allie verify` in real app repos and preserve receipts that show map,
   evidence, report, release, JUnit, and SARIF artifacts.
3. Use the competitive landscape to prioritize packet provenance,
   replayability, privacy governance, and criterion-by-surface drilldown.

## Next

1. Make repository-only cold starts useful without requiring a manifest, then
   deepen them through sandboxed staged-app, authenticated, and changed-surface
   discovery.
2. Add PR/check publishers as adapters over the existing reporter outputs,
   without duplicating policy in CI-specific files.

Before adding dashboard, browser-cloud, or AI-heavy work, refresh
[competitive-landscape.md](competitive-landscape.md) and prefer roadmap slices
that improve packet provenance, replayability, privacy governance, and
criterion-by-surface drilldown over scanner-rule parity.

## Later

1. Enable approved live multimodal provider calls behind the model gateway.
2. Wire GitHub Checks, PR comments, Azure, and other publishers from the same
   packets.
3. Ingest externally authored, append-only reviewer attestations and waivers;
   keep the authoring workflow outside Allie.
4. Let later viewers, dashboards, or investigation tools consume the portable
   packets without becoming required Allie infrastructure.

## Code Health Backlog

Behavior-preserving refactors logged from code review; sequence as capacity allows.

1. **Decompose `src/lib.rs`** (~10k lines, 271 fns / 82 structs, no internal
   modules beyond `consumer`). Every feature appends to one god-module; the
   seams are already cohesive clusters — e.g. `compliance::model`
   (`build_compliance_report`/`compliance_obligation`/`criterion_coverage_*`/
   `compliance_summary`), `compliance::report` (the `cr_*` helpers +
   `render_compliance_report` + the inlined `REPORT_CSS`), `agentic`
   (`run_agentic_review`/`agentic_promoted_status`), `evidence`, `discovery`.
2. **Typed `Verdict { Pass, Fail, Inconclusive }`** (with `FromStr`) to retire
   the "is this verdict settled?" rule currently triplicated across the worker,
   the Rust ingest, and `agentic_promoted_status`, and to replace stringly-typed
   status/confidence matching at the boundary.
3. **Extract `captureFrames`** in `workers/agentic/review.mjs` mirroring
   `recordClip`, so the motion-montage capture scaffold (open → goto → act →
   close) lives once instead of a third hand-inlined spelling.

## First Acceptance Slice

The first slice is complete when this command works against a checked-in fixture:

```sh
allie run --manifest examples/login-flow.yml --out .allie/runs/latest
```

Required evidence:

- JSON packet;
- HTML report;
- Playwright route state;
- axe results;
- at least one screenshot;
- deterministic exit code;
- replay instructions.
