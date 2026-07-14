# Allie Product Spec

Date: 2026-07-14

## Product Thesis

Allie is point-and-shoot accessibility release intelligence: a contained,
unattended harness that starts from repository access, proves what can be proven
automatically, accounts explicitly for what it could not prove, and keeps
accessibility evidence tied to code changes and release decisions.

The core product is an evidence system, not a score and not a generic scanner.

## Target Users

- Release-owning frontend and full-stack engineers who need fast, reproducible
  feedback in CI and PRs.
- QA engineers who need browser-flow coverage and regression evidence.
- Accessibility engineers who review PRs and need a report deep enough to launch
  their own investigations without repeating Allie's work.
- Product and engineering leaders who need release status and trends.
- Compliance and audit stakeholders who need defensible evidence packets.

## Job To Be Done

Given a repository and whatever runnable environment and policy context are
available, autonomously discover and test the meaningful accessibility surface;
produce a complete standards obligation ledger with evidence or attempted-method
diagnostics behind every status; identify what blocks release; and generate one
portable report that release engineers and accessibility engineers can trust.

## Non-Goals

- Allie is not a legal compliance guarantee.
- Allie is not an autonomous replacement for expert or lived accessibility review.
- Allie is not a dashboard detached from CI and release workflows.
- Allie is not a scanner that collapses the product into one vague score.
- Allie is not SaaS-first before local replay and evidence contracts are stable.
- Allie does not remediate, suggest fixes, apply patches, or own a remediation
  queue.
- Allie is not a conversational agent or interactive accessibility workbench.
- Allie does not require GitHub, a hosted account, or a hand-authored product map.

## Core Bets

1. Replayable evidence is more valuable than one-time findings.
2. Deterministic failures should block regressions.
3. Agentic and multimodal findings should enrich and prioritize, not pretend certainty.
4. Human review is part of the system, not a failure of automation.
5. Rust should own orchestration, schemas, policy, storage, and reproducibility.
6. Playwright and axe should run behind a narrow worker boundary.
7. OpenRouter/model routing is useful only behind strict provider, privacy, budget, and audit policy.
8. Repository inspection and runtime exploration should discover context before
   asking consumers to enumerate routes, journeys, themes, tenants, roles, or
   states.
9. Durable test intent belongs in a versioned accessibility test plan; generated
   executable tests belong in an ephemeral isolated workspace, not the target
   repository. A test-only patch may be emitted as an inert report artifact, but
   the audit never applies it and never proposes product remediation.
10. A complete report and a complete assessment are different. The obligation
    ledger is total; evaluated scope, sampling, failed attempts, and unverified
    cells remain explicit.
11. Local files, GitHub, Azure, object storage, and future systems are publisher
    adapters over one canonical artifact bundle.

## Point-and-Shoot Product Contract

The canonical invocation starts from a checkout without requiring prior Allie
state or a manifest. An optional manifest or prior packet may add policy,
credentials, known journeys, environment adapters, or historical context.

The unattended run must:

1. Resolve a read-only target snapshot and record its revision and provenance.
2. Infer build, launch, surfaces, routes, roles, themes, tenants, workflows,
   states, and variation candidates from source and runtime evidence.
3. Merge optional caller context through one explicit precedence policy.
4. Emit a versioned accessibility test plan covering surfaces, states,
   variations, obligations, methods, provenance, sampling, and unresolved gaps.
5. Generate and run appropriate static, unit, integration, Playwright/axe, and
   agentic checks inside a declared sandbox without mutating the target.
6. Enforce per-run limits for elapsed time, spend, calls, actions, retries,
   variants, and artifact bytes, then emit a usage and stop-reason receipt.
7. Produce a complete obligation ledger. Every status carries evidence, or an
   attempted method, diagnostic, missing prerequisite, and residual inquiry.
8. Write one canonical packet and progressively disclosed report bundle that a
   publisher adapter can deliver without changing accessibility semantics.
9. Return a blocking exit code only for deterministic/scripted policy failures
   or required evidence gaps; model opinion alone never blocks.

## Standards Model

Allie models standards as obligation profiles, not a single pass/fail score.

Initial profiles:

- `wcag21-aa`
- `wcag22-aa`
- `section508`
- `ada-title-ii`
- client-specific policy packs

`wcag22-aa` is the primary V0 ledger. Reports may project that ledger into a
`wcag21-aa` view for EAA/EN 301 549 consumers by excluding WCAG 2.2-only
criteria and explicitly surfacing WCAG 2.1-only legacy gaps such as 4.1.1
Parsing. That projection is evidence visibility, not a legal conformance claim.

Evidence classes:

- `deterministic`: axe, static DOM/CSS/AST, contrast math, labels, roles, headings, language, caption metadata.
- `scripted`: Playwright flows, keyboard traversal, focus management, modal/menu behavior, form error recovery, zoom/reflow, reduced motion.
- `agentic`: screenshot/video review, visual order, focus visibility, ambiguous copy, target affordance, alt usefulness, motion assessment.
- `human`: expert/lived review, equivalent access, content meaning, cognitive load, captions/audio description quality, waiver decisions.

Verdict statuses:

- `pass`
- `fail`
- `not_applicable`
- `needs_review`
- `not_tested`
- `waived`
- `risk_accepted`

Confidence/provenance:

- `machine_proven`
- `script_observed`
- `agent_inferred`
- `human_attested`

## PR and Release Gate Policy

Block on:

- new deterministic failures in a required profile;
- scripted critical-flow failures;
- missing required evidence for changed high-risk surfaces;
- expired waivers on touched surfaces.

Warn or require review on:

- agentic low/medium-confidence findings;
- new `needs_review` obligations;
- reduced route or standard coverage;
- stale evidence;
- changed product surfaces with no mapped journey.

Never block on:

- pure model opinion without evidence;
- criteria Allie cannot currently test and has not marked as required human review;
- a global score detached from specific obligations and product surfaces.

## Evidence Packet Requirements

Every run should produce an evidence packet with:

- run id, tool version, git metadata, environment metadata, timestamp, and policy profile;
- resolved-context provenance, optional manifest/prior-packet references, and
  discovered route/surface/variation coverage;
- accessibility test-plan version, methods, selection/sampling decisions, and
  unresolved discovery;
- browser, viewport, mobile-web viewport, color scheme, reduced-motion, zoom, and locale settings;
- axe JSON and summarized deterministic findings for captured desktop and mobile-web passes where configured;
- DOM and accessibility tree snapshots for inspected states;
- screenshots and optional video/GIF clips;
- console/network summaries with sensitive data redacted;
- model prompts, model ids, provider metadata, redaction state, and model outputs for agentic review;
- verdicts mapped to standards obligations;
- waivers and human-review provenance;
- isolation, sanitization, redaction, model-egress, usage, budget, and
  methodology receipts;
- attempted-method diagnostics, missing prerequisites, and residual inquiry for
  every unverified obligation;
- replay instructions.

## Security and Privacy Contract

Allie assumes staging apps may still contain sensitive data.

Requirements:

- Execute target code and generated tests only through a declared ephemeral
  isolation boundary with a read-only target, scratch-only writes, scoped
  filesystem access, deny-by-default egress, bounded resources, process-tree
  teardown, and brokered credentials. Unguarded runs are not release-grade.
- Use short-lived scoped credentials.
- Store secrets only through explicit credential providers.
- Classify artifacts at creation; redact screenshots, DOM, console logs, network
  summaries, prompts, URLs, and related metadata before model egress or public
  publication when policy requires it.
- Route real customer-like data only to approved ZDR-capable providers.
- Never silently fall back from approved providers to unapproved providers.
- Keep audit logs for model calls, generated findings, evidence projections, PR comments, and waiver decisions.
- Bound exploration by route, time, spend, screenshot/video count, model calls, and retry limits.
- Treat production-like environment provisioning and data sanitization as a
  caller-owned adapter contract. Record its attestation; never acquire raw
  production extraction credentials by default.

## Shipped V0 Foundation

The original manifest-first acceptance slice remains the regression foundation:

```sh
allie run --manifest examples/login-flow.yml --out .allie/runs/latest
```

And it produces:

- a machine-readable evidence packet;
- a local HTML report;
- at least one Playwright-driven route state;
- axe results mapped to a standards profile;
- deterministic pass/fail exit behavior;
- a documented replay command;
- `cargo test --locked` green.

It proves the packet pipeline, not the full point-and-shoot contract above.
