# Allie Vision

> Accessibility evidence for every release.

Allie is an open-source, self-hosted accessibility evidence harness: a Rust-first
agentic engine you run as a developer, against your own repository, to assess
where an application stands against accessibility standards — and then generate
deep, comprehensive, replayable context you can act on however you like.

You bring your own config, your own model, and your own API keys. You clone or
download Allie into whatever repo you are working in, point it at your app, and
run it. Wire it into CI, run it locally before a release, or drive it by hand.
There is no account to create and nothing sent to a vendor by default. Allie runs
where your code runs.

Allie's job is autonomous accessibility audit, mapping, and reporting. It
discovers the codebase, product surfaces, routes, themes, workflows, and likely
user stories; writes and runs the tests and agentic QA loops itself; analyzes
screenshots, GIFs, and video walkthroughs with vision models; proves what can be
proven automatically; and renders a defensible, replayable picture of
accessibility status tied to a specific code revision.

Allie does not remediate. Fixing what the audit finds — by hand, by ticket, or by
a downstream fixing agent — is a separate product that consumes Allie's evidence.
Allie's contribution is that the report is already deep, structured, and
replayable when the fix begins, so nothing has to be re-investigated.

This is the project north star. `SPEC.md` is the product contract — acceptance
model, gate policy, and packet schema. When the two disagree, the vision says
what we are trying to build and the spec says what we have agreed to ship; close
the gap rather than letting either drift.

## Audience

- Frontend and full-stack engineers who want fast, reproducible accessibility
  feedback in CI and pull requests, tied to the code that changed.
- QA engineers who need real browser-flow evidence for keyboard, focus,
  zoom/reflow, reduced-motion, modal, menu, and form-error behavior.
- Accessibility specialists who want richer context and far less manual setup
  before they apply judgment.
- Product, engineering, compliance, and audit stakeholders who need a defensible
  release picture without a false legal guarantee.

The first reader of an Allie report is increasingly another agent. The artifact
is written to be trusted and acted on by a machine as readily as by a person.

## The Job

Given a repository, a running or staged app, and a policy profile, Allie should
discover the sitemap, product surfaces, likely user stories, relevant config,
themes, and interaction states; write and replay deterministic Playwright + axe
coverage through a real browser; map every result to its relevant WCAG 2.2 A/AA
obligation; run agentic vision passes over screenshots, GIFs, and video
walkthroughs that render a committed pass/fail verdict (shown asterisked, with
the evidence inlined) on criteria that require judgment; and produce a report
where every in-scope criterion is green, red, not applicable, or explicitly
unverified across every discovered product surface.

The output is the product. It is deep and comprehensive by default, generated
automatically, and structured so the next agent — or the next human — can trust
it.

## What Must Stay True

These are invariants, not features. They define what Allie *is*; if a change
breaks one of them, it is the wrong change regardless of how convenient it is.

- **Evidence over assertion.** Every status traces to a replayable artifact. No
  claim ships that Allie cannot show its work for.
- **No silent gaps.** Every in-scope criterion gets an explicit status. "Not
  tested" is visible and honest; it is never dressed up as a pass and never
  quietly omitted.
- **Surface coverage is part of the claim.** A green criterion is meaningful
  only for the routes, states, themes, viewports, and workflows Allie actually
  mapped and exercised.
- **Honest uncertainty.** Deterministic certainty and model judgment never blend
  into one fake number. A machine-certain failure, a scripted result, an agentic
  verdict, and a human attestation are different kinds of knowing, and the report
  keeps them distinguishable.
- **Provenance and replay.** Every verdict is tied to a code revision and can be
  re-run. Results are reproducible artifacts, not point-in-time screenshots that
  rot.
- **Local-first and yours.** Allie runs where your code runs, on your keys, with
  nothing sent to a vendor by default. Privacy, provenance, and replayability are
  product features, not later hardening.
- **No legal promise.** Allie reports evidence, status, confidence, and residual
  review needs. It never claims legal compliance and never implies that a green
  run discharges a legal obligation.

## What Allie Is Not

Allie is not a scanner, a single score, a generic agent platform, or a legal
compliance promise. It is an evidence system whose primary interface is a set of
versioned packets tied to discovered surfaces, generated flows, policy profiles,
code revisions, artifact sets, review attempts, diagnostic context, and replay
commands.

The repo should refuse work that erodes the invariants above, specifically:

- A global accessibility "score" or grade that collapses the obligation ledger
  into one number.
- Remediation of any kind — fix suggestions, patch generation, or a remediation
  queue. Allie audits, maps, and reports; fixing what it finds is a separate
  product that consumes Allie's evidence. The audit report is where Allie's job
  ends.
- Model-only findings that block a release on their own. Agentic verdicts inform
  and prioritize; they gate only after scripted reproduction or human
  attestation promotes them.
- A default cloud dependency, telemetry, or hosted account requirement bolted
  onto the local path.
- Provider or browser details leaking outside their boundaries — model routing
  outside the gateway, Playwright/axe specifics outside the worker adapter.
- Infrastructure-specific coupling in the core report contract. GitHub Actions
  may be the common integration path, but CI, nightly, weekly, staging, and
  local runs should all consume the same evidence model.

## Strategic Bets

These are wagers about *how* to win the job. Unlike the invariants above, a bet
can be revisited if evidence says we bet wrong.

1. Autonomy should be packeted and replay-gated: discovery packet -> sitemap and
   story packet -> flow-plan packet -> generated E2E candidate -> evidence packet
   -> release decision. Each step is an inspectable artifact, not a black box.
2. A complete WCAG obligation ledger is the reporting spine. Each criterion needs
   status, scope, method, artifacts, confidence, residual review, and diagnostic
   context instead of a global score.
3. Committed agentic verdicts beat "needs review." Judgment-heavy criteria should
   read as decisions — a pass/fail rendered from inlined evidence (asterisked) —
   rather than punting to a human queue, while still staying advisory for gating
   until promoted by scripted reproduction or attestation.
4. Rust owns the durable core: orchestration, schemas, policy, budgets, hashing,
   storage, promotion state, and release enforcement. Browser automation lives
   behind a narrow Playwright/axe worker boundary; model and vision calls live
   behind a typed gateway with redaction receipts, provider allowlists,
   ZDR/no-fallback policy, prompt versions, and audit events.
5. The report is the handoff. Allie generates the audit context so completely and
   so structured that whatever comes next — a fix, a ticket, an auditor sign-off —
   starts from evidence, not a fresh investigation. Allie does not take that next
   step itself.
6. CI and scheduled verification are first-class distribution paths. Pull
   requests should show what accessibility changed, while nightly or weekly
   staging runs answer whether the shipped product is still accessible.

## Lifespan

Allie is built to last. It is a long-lived, public, open-source project meant to
be durable developer and product substrate — cloned into any repository, wired
into real release workflows, and dogfooded internally against our own apps. That
sets a high maintenance bar: the local evidence, schema, and replay contracts are
the whole value proposition, so they are versioned, kept stable, and broken only
deliberately.

Allie is not SaaS-first. A managed or hosted layer — multi-tenant dashboards,
shared history, organization rollups — is a plausible later evolution, not a
near-term goal, and nothing in the hosted direction is allowed to compromise the
local-first contracts it would eventually be built on.

## What Excellent Looks Like

**Now.** The local loop is trustworthy and boring to run. A developer runs one
command against a staged app and gets a replayable evidence packet, a WCAG
drilldown with no silent gaps, and a release decision that explains itself —
every time, byte-stable enough to gate CI on.

**Six to twelve months.** A developer clones Allie into their repo, points it at
a staged or running app with their own model keys, and runs one command in CI or
locally. Allie autonomously discovers a representative product surface; generates
and replays Playwright plus axe coverage; preserves DOM, accessibility tree,
screenshot, video/GIF, trace, console, network, and model-review artifacts under
redaction policy; and emits a WCAG drilldown report that shows every criterion as
pass, fail, not applicable, or a committed agentic verdict — with no silent "not
tested" gaps. The report is complete enough that whatever fixes the findings — an
agent or a human — can start without re-investigating. It explains every
release-blocking decision in terms of evidence, and never claims legal compliance.

**Beyond.** Allie is the default way a team brings accessibility evidence to a
release conversation — the harness whose packets a fixing agent, an auditor, and
a release manager all trust without re-litigating the findings. The category
shifts from "we ran a scanner once" to "every release carries reproducible
accessibility evidence," and Allie is the reference implementation of what that
evidence should contain.
