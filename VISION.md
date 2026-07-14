# Allie Vision

> Accessibility evidence for every release.

Allie is an Apache-2.0 open-source, self-hosted accessibility evidence harness: a Rust-first
agentic engine you run as a developer, against your own repository, to assess
where an application stands against accessibility standards — and then generate
deep, comprehensive, replayable context you can act on however you like.

The canonical entrypoint is the repository itself. You clone or download Allie,
point it at a checkout, and dispatch one unattended run. Allie infers what it can
from source and runtime; an optional manifest adds policy, credentials, required
journeys, environment adapters, budgets, and organizational constraints. Wire it
into CI or run it locally before a release. There is no account to create and
nothing sent to a vendor by default. Allie runs where your code runs.

Allie's job is autonomous accessibility audit, mapping, testing, and reporting.
It discovers the codebase, product surfaces, routes, roles, themes, tenants,
workflows, states, and meaningful variations; designs and maintains a compact
accessibility test plan; generates and runs the required static, unit,
integration, browser, and agentic QA in an isolated workspace; analyzes
screenshots, GIFs, and video walkthroughs with vision models; proves what can be
proven automatically; and renders a defensible, replayable picture of
accessibility status tied to a specific code revision. Generated executable
tests do not mutate the target repository. A proposed permanent test patch may
be emitted as an artifact, but applying it is a separate explicit action.

Allie does not remediate. Fixing what the audit finds — by hand, by ticket, or by
a downstream fixing agent — is a separate product that consumes Allie's evidence.
Allie's contribution is that the report is already deep, structured, and
replayable when the fix begins, so nothing has to be re-investigated.

This is the project north star. `SPEC.md` is the product contract — acceptance
model, gate policy, and packet schema. When the two disagree, the vision says
what we are trying to build and the spec says what we have agreed to ship; close
the gap rather than letting either drift.

## Audience

- Frontend and full-stack engineers who own releases and want fast,
  reproducible accessibility feedback in CI and pull requests, tied to the code
  that changed.
- QA engineers who need real browser-flow evidence for keyboard, focus,
  zoom/reflow, reduced-motion, modal, menu, and form-error behavior.
- Accessibility engineers who review pull requests and need reports rich enough
  to become the context and jumping-off point for their own investigations.
- Product, engineering, compliance, and audit stakeholders who need a defensible
  release picture without a false legal guarantee.

Release-owning engineers and accessibility engineers are co-primary users. They
share one canonical report: progressive disclosure should make the release
picture immediate without hiding criterion-, surface-, finding-, and
artifact-level depth. The first reader is also increasingly another agent, so
the artifact must be trusted and acted on by a machine as readily as by a
person.

## The Job

Given repository access — plus a runnable or staged app, credentials, policy,
and environment context when available — Allie should discover the sitemap,
product surfaces, likely user stories, relevant config, themes, roles, tenants,
and interaction states; generate and replay deterministic Playwright + axe and
other appropriate coverage; map every result to its relevant WCAG 2.2 A/AA
obligation; run agentic vision passes over screenshots, GIFs, and video
walkthroughs that render a committed pass/fail verdict (shown asterisked, with
the evidence inlined) on criteria that require judgment; and produce a report
where every in-scope criterion is green, red, not applicable, or explicitly
unverified across every discovered product surface.

The report is always complete even when the assessment cannot be. An
unreachable workflow or unavailable form of evidence remains a visible ledger
cell with attempted method, failure diagnostics, missing prerequisite, and
residual investigation — never an omitted obligation and never a fabricated
pass. Allie runs as comprehensively as the available context and configured
budgets allow.

The output is the product. It is deep and comprehensive by default, generated
automatically, and structured so the next agent — or the next human — can trust
it.

The V0 target is web content and web applications. Mobile web is not a separate
non-goal: WCAG applies across responsive web variations, so Allie captures
mobile viewport evidence for web states and keeps mobile-relevant WCAG criteria
in the ledger. Native mobile apps (iOS/Android apps that are not web content)
are outside the current product scope until Allie has a native-app runner and
standard-specific evidence model.

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
- **Point and shoot.** Dispatch Allie, leave it alone, and receive the finished
  evidence and report. Agentic work is internal and unattended; Allie is not a
  conversational or drivable investigation tool.
- **Cold-start capable.** Every run can begin from repository access alone.
  Prior plans, packets, and human review records are optional explicit inputs,
  never hidden memory or required infrastructure.
- **Read-only target.** Allie may inspect, clone, build, and run target code in
  a declared sandbox, but the audit does not rewrite the target repository.
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
- Product remediation of any kind — fix suggestions, product-code patches, or a
  remediation queue. Allie audits, maps, and reports; fixing what it finds is a
  separate product that consumes Allie's evidence. A test-only patch may be
  proposed as an inert report artifact under the rule above, but Allie never
  applies it and never proposes a product fix. The audit report is where
  Allie's job ends.
- An interactive accessibility workbench. Accessibility engineers investigate
  from Allie's output in tools of their choice. Allie may ingest portable,
  append-only attestations, waivers, corrections, and review notes on a later
  run, but it does not own the workflow that authors them.
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
   requests should deeply assess changed and high-risk surfaces, while nightly,
   weekly, and release runs refresh the broadest feasible application coverage.
   Both use the same evidence contract; carried-forward, stale, sampled, and
   untested coverage stays explicit. When evaluating every discovered state is
   infeasible, selection follows a reproducible methodology aligned with
   WCAG-EM: defined scope and conformance target, structured and random samples,
   complete-process closure, and a methodology receipt. Sampled evidence never
   becomes a whole-product conformance claim.
7. Discovery should remove setup, not conceal uncertainty. Repository inspection
   is the zero-config baseline; runtime exploration and optional consumer context
   deepen it. Consumers may seed critical journeys and known dimensions, but do
   not have to enumerate every nook, theme, tenant, role, or state before Allie
   is useful.
8. Test intent outlives generated test code. The durable accessibility test plan
   records surfaces, states, variants, obligations, methods, and evidence needs.
   Executable tests are generated ephemerally behind adapters and can be
   regenerated from the plan.
9. Context and publication are adapter boundaries. A target adapter supplies a
   pinned read-only checkout or runnable application; an environment adapter may
   provision production-like data under an organization-owned sanitization
   policy and return a machine-readable attestation; publishers deliver the same
   canonical bundle to local disk, GitHub, Azure, object storage, or future
   systems. GitHub is the first path, not a core assumption.
10. Comprehensiveness is bounded and auditable. Core policy enforces configurable
    per-run ceilings for time, spend, model calls, browser actions, retries,
    variants, and artifact volume. Coverage saturation, sampling decisions,
    budget exhaustion, and every remaining gap are receipts, not invisible
    control flow. Cross-run
    budget accounting is a later adapter, not required core state.
11. Running arbitrary target code requires declared isolation. Builds,
    application processes, generated tests, and exploration execute in an
    ephemeral sandbox with scoped filesystem access, bounded resources,
    deny-by-default egress, and brokered credentials. An unguarded run must say
    so and cannot present itself as release-grade.

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
local-first contracts it would eventually be built on. Portable packets and
publisher adapters are the extension point; the core does not grow a hosted
control plane to achieve distribution.

## What Excellent Looks Like

**Now.** The shipped manifest-first local loop is trustworthy and boring to run.
It returns a replayable evidence packet, a WCAG drilldown with no silent gaps,
and a release decision that explains itself, byte-stable enough to gate CI on.
The immediate product target is to preserve those properties while replacing
required hand-authored manifests with one unattended repository dispatch.

**Six to twelve months.** A developer dispatches Allie against an arbitrary web
repository with no required hand-authored product map. Allie inspects source,
starts or connects to the application through a sandboxed adapter, discovers
meaningful surfaces and variation dimensions, maintains the compact test plan,
and generates and replays the necessary coverage. It preserves DOM,
accessibility tree, screenshot, video/GIF, trace, console, network, test, and
model-review artifacts under redaction policy; then emits a portable WCAG
drilldown report that accounts for every criterion and every discovered surface
without silent gaps. Pull-request runs focus on change; scheduled and release
runs evaluate the broadest feasible scope and disclose any reproducible
sampling. The report is complete enough that whatever investigates or
fixes the findings — an accessibility engineer, another agent, or a product team
— can start without repeating Allie's work. It explains every release-blocking
decision in terms of evidence and never claims legal compliance.

**Beyond.** Allie is the default way a team brings accessibility evidence to a
release conversation — the harness whose packets a fixing agent, an auditor, and
a release manager all trust without re-litigating the findings. The category
shifts from "we ran a scanner once" to "every release carries reproducible
accessibility evidence," and Allie is the reference implementation of what that
evidence should contain.
