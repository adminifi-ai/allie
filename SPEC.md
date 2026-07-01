# Allie Product Spec

Date: 2026-06-18

## Product Thesis

Allie is accessibility release intelligence: a harness that proves what can be proven automatically, packages context for what still needs judgment, and keeps accessibility evidence tied to code changes and release decisions.

The core product is an evidence system, not a score and not a generic scanner.

## Target Users

- Frontend engineers who need fast, reproducible feedback in CI and PRs.
- QA engineers who need browser-flow coverage and regression evidence.
- Accessibility specialists who need richer context and fewer manual setup steps.
- Product and engineering leaders who need release status and trends.
- Compliance and audit stakeholders who need defensible evidence packets.

## Job To Be Done

Before a release ships, prove which critical user journeys are accessible, identify what blocks release, explain what remains uncertain, and generate enough context for engineering, product, accessibility, and compliance stakeholders to trust the decision.

## Non-Goals

- Allie is not a legal compliance guarantee.
- Allie is not an autonomous replacement for expert or lived accessibility review.
- Allie is not a dashboard detached from CI and release workflows.
- Allie is not a scanner that collapses the product into one vague score.
- Allie is not SaaS-first before local replay and evidence contracts are stable.

## Core Bets

1. Replayable evidence is more valuable than one-time findings.
2. Deterministic failures should block regressions.
3. Agentic and multimodal findings should enrich and prioritize, not pretend certainty.
4. Human review is part of the system, not a failure of automation.
5. Rust should own orchestration, schemas, policy, storage, and reproducibility.
6. Playwright and axe should run behind a narrow worker boundary.
7. OpenRouter/model routing is useful only behind strict provider, privacy, budget, and audit policy.

## First Product Wedge

Build a CLI plus CI check that can:

1. Read a flow manifest for one staged app.
2. Authenticate with supplied staging credentials.
3. Execute one critical user journey in Playwright.
4. Run axe checks on page and interaction states.
5. Capture screenshots, DOM snapshots, accessibility tree snapshots, console/network summaries, and trace metadata.
6. Write an evidence packet to disk.
7. Produce a local HTML report.
8. Return a blocking exit code only for deterministic or scripted required failures.

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
- flow manifest id and route/surface coverage;
- browser, viewport, mobile-web viewport, color scheme, reduced-motion, zoom, and locale settings;
- axe JSON and summarized deterministic findings for captured desktop and mobile-web passes where configured;
- DOM and accessibility tree snapshots for inspected states;
- screenshots and optional video/GIF clips;
- console/network summaries with sensitive data redacted;
- model prompts, model ids, provider metadata, redaction state, and model outputs for agentic review;
- verdicts mapped to standards obligations;
- waivers and human-review provenance;
- replay instructions.

## Security and Privacy Contract

Allie assumes staging apps may still contain sensitive data.

Requirements:

- Use short-lived scoped credentials.
- Store secrets only through explicit credential providers.
- Redact screenshots, DOM, console logs, network summaries, and prompts where configured.
- Route real customer-like data only to approved ZDR-capable providers.
- Never silently fall back from approved providers to unapproved providers.
- Keep audit logs for model calls, generated findings, evidence projections, PR comments, and waiver decisions.
- Bound exploration by route, time, spend, screenshot/video count, model calls, and retry limits.

## Acceptance For V0

V0 is acceptable when this command exists:

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
