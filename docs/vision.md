# Allie Vision

Allie is accessibility release intelligence: a Rust-first evidence harness that
turns critical user journeys into replayable accessibility proof for release
decisions.

## Audience

- Frontend engineers who need fast, deterministic feedback in CI and pull
  requests.
- QA engineers who need browser-flow regression evidence.
- Accessibility specialists who need richer context and fewer manual setup
  chores.
- Product, engineering, compliance, and audit stakeholders who need defensible
  release packets without a false legal guarantee.

## Job

Before a release ships, Allie proves what can be proved automatically, shows
what still needs judgment, and preserves enough context for a reviewer to trust
or challenge the release decision.

## Category

Allie is not a scanner, score, dashboard, or legal compliance promise. It is an
evidence system whose primary interface is a versioned evidence packet tied to a
specific flow, policy profile, code revision, artifact set, and replay command.

## Strategic Bets

1. A replayable local evidence loop should exist before SaaS, PR automation, or
   model review.
2. Deterministic and scripted failures can block releases; model-only findings
   enrich review and should not block by themselves.
3. Rust should own orchestration, schemas, policy, storage, and reproducibility.
4. Browser automation belongs behind a narrow Playwright/axe worker boundary.
5. Privacy and trust boundaries are product features, not later hardening.

## Six To Twelve Month Target

Allie can run in CI against the critical flows of a staged app, produce
standards-mapped evidence packets and local HTML reports, preserve redacted
artifacts with hashes and replay instructions, explain deterministic release
blocking decisions, and route uncertain obligations to human or agent-assisted
review without claiming legal compliance.

