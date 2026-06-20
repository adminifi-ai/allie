# Roadmap

## Current V0 Loop

The first local evidence loop is implemented around:

```sh
cargo run --locked -- run --manifest examples/login-flow.yml --out .allie/runs/latest
```

It reads the checked-in login manifest, serves the local fixture through the
browser worker, runs Playwright plus axe, captures a screenshot, writes raw axe
JSON, emits an `allie.evidence.v0` packet, and generates a local HTML report.

This is a proof foundation, not the autonomous product. The current system does
not yet discover application surfaces, generate tests, run vision AI, complete a
WCAG matrix, or draft remediation.

## Product Target

Allie should let a compliance engineer point it at an application and receive:

1. An autonomously discovered sitemap, product-surface inventory, and likely user
   stories.
2. Generated Playwright and axe coverage that replays through real browser
   evidence before it can enforce release policy.
3. A complete WCAG 2.2 A/AA obligation ledger with drilldown from criterion to
   state, finding, artifact, agentic context, waiver, and remediation.
4. Agentic vision review for criteria that require judgment, with redaction
   receipts and neutral findings until promoted by scripted proof or human
   attestation.
5. Release enforcement and remediation guidance that are packet projections, not
   a separate status model.

## Now

1. Replace the synchronous advisory agent helper with durable autonomous jobs
   (`backlog.d/011-replace-synchronous-agent-helper-with-durable-jobs.md`).
   A 120-second subprocess timeout is acceptable as a smoke guard, not as the
   product contract for agentic assessment.
2. Make WCAG reporting an exact 55-criterion surface matrix
   (`backlog.d/012-make-wcag-coverage-a-55-criterion-surface-matrix.md`).
   Supporting checks and aggregate gates should explain evidence, not inflate
   the standards denominator.
3. Keep the autonomous workbench smoke green as the primary regression oracle
   while the V1 job and coverage contracts land (`npm run autonomous:smoke`).

## Next

1. Package the host-agnostic consumer contract with `allie init`, `allie verify`,
   stable reporter outputs, and GitHub/Azure examples as thin wrappers
   (`backlog.d/014-package-host-agnostic-consumer-contract.md`).
2. Track the accessibility tooling landscape as product input so Allie
   differentiates on evidence contracts, replayability, governance, and release
   semantics rather than scanner parity
   (`backlog.d/013-track-accessibility-tooling-landscape.md`).
3. Add authenticated staged-app discovery and changed-surface inference once the
   job and coverage contracts are explicit.

## Later

1. Enable approved live multimodal provider calls behind the model gateway.
2. Add richer remediation patch adapters, before/after packet comparison, and
   reviewer attestations.
3. Wire GitHub Checks, PR comments, and hosted evidence viewer from the same
   packets.
4. Add SME review workbench, reviewer attestations, and promotion workflows.
5. Add browser extension capture companion, multi-repo dashboard, and trends.

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
