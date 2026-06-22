# Allie Vision

Allie is an autonomous accessibility compliance workbench: a Rust-first evidence
harness that discovers an application's product surface, generates replayable
accessibility coverage, enriches judgment-heavy checks with agentic review, and
turns the result into enforceable release evidence.

## Audience

- Accessibility compliance engineers who want to point Allie at an application
  and get as much autonomous assessment, remediation planning, enforcement, and
  reporting as possible.
- Frontend engineers who need generated Playwright, axe, and regression coverage
  tied to source-adjacent remediation guidance.
- QA engineers who need browser-flow evidence for keyboard, focus, zoom/reflow,
  reduced-motion, modal, menu, and form-error behavior.
- Product, engineering, compliance, and audit stakeholders who need defensible
  release packets without a false legal guarantee.

## Job

Given a repo, staged app, credential profile, and policy pack, Allie should
discover the sitemap, product surfaces, and likely user stories; generate and
replay accessibility tests; map evidence to every relevant WCAG obligation; take
agentic vision passes that render a committed pass/fail verdict (shown with an
asterisk) on criteria that require judgment; and produce a report
where a compliance engineer can drill from standard to finding, test, artifact,
context, waiver, remediation, and release decision.

## Category

Allie is not a scanner, score, generic agent platform, or legal compliance
promise. It is an evidence system whose primary interface is a set of versioned
packets tied to discovered surfaces, generated flows, policy profiles, code
revisions, artifact sets, review attempts, remediation actions, and replay
commands.

## Strategic Bets

1. Autonomy should be packeted and replay-gated: discovery packet -> sitemap and
   story packet -> flow-plan packet -> generated E2E candidate -> evidence packet
   -> release decision.
2. A complete WCAG obligation ledger is the reporting spine. Each criterion
   needs status, scope, method, artifacts, confidence, residual review, and
   remediation context instead of a global score.
3. Deterministic and scripted failures can block releases. Agentic vision
   verdicts render a committed pass/fail (asterisked, with the evidence inlined)
   so judgment-heavy criteria read as decisions rather than "needs review", but
   they stay advisory for gating: they do not block a release until promoted by
   scripted reproduction or human attestation.
4. Rust should own orchestration, schemas, policy, budgets, hashing, storage,
   promotion state, and release enforcement.
5. Browser automation belongs behind a narrow Playwright/axe worker boundary;
   model and vision calls belong behind a typed gateway with redaction receipts,
   provider allowlists, ZDR/no-fallback policy, prompt versions, and audit events.
6. Privacy, provenance, and replayability are product features, not later
   hardening.

## Six To Twelve Month Target

Allie can run in CI or locally against a staged application, autonomously
discover a representative product surface, generate and replay Playwright plus
axe coverage, preserve DOM, accessibility tree, screenshot, video/GIF, trace,
console, network, and model-review artifacts under redaction policy, and emit a
WCAG drilldown report that shows every criterion as pass, fail, not applicable,
not tested, or needs review. It can draft remediation plans and branch-scoped
patch attempts only when tied to evidence references and replay commands. It
explains release blocking decisions without claiming legal compliance.
