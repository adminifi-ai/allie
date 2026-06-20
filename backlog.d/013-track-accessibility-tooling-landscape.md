# Track the accessibility tooling landscape as product input

Priority: P1 - Status: ready - Estimate: L

## Goal

Maintain a sourced competitive landscape that keeps Allie differentiated from
scanner, dashboard, manual-audit, and browser-cloud alternatives by comparing
evidence contracts, replayability, standards visibility, agentic governance, and
release-decision semantics.

## Oracle

- [ ] `docs/competitive-landscape.md` compares Deque/axe, Evinced, Level Access,
  Accessibility Insights, Pa11y, Lighthouse, BrowserStack, Sauce Labs, Stark,
  and notable alternatives using the same rubric.
- [ ] Each entry cites current sources and records product category, automation
  depth, WCAG/standards claims, CI/PR integration, evidence artifacts, replay
  support, manual review workflow, AI/agentic claims, privacy/governance posture,
  and Allie differentiation.
- [ ] A recurring review checklist exists so major roadmap changes re-check
  whether a competitor already solves the proposed value.
- [ ] Backlog prioritization uses the landscape to prefer capabilities that
  strengthen Allie's unique evidence/replay/governance position.

## Verification System

- Claim: Allie's strategy is informed by current alternatives instead of
  drifting into a weaker clone of existing scanners or enterprise dashboards.
- Falsifier: competitor claims without sources, stale entries older than the
  review cadence, missing major tools, or backlog epics that ignore the
  landscape rubric.
- Driver: periodic web research plus doc lint that verifies every row has at
  least one source URL and an Allie-differentiation note.
- Grader: landscape completeness checklist, source freshness dates, and roadmap
  cross-reference checks.
- Evidence packet: `docs/competitive-landscape.md` plus research receipts under
  `.allie/research/landscape/` when available.
- Cadence: before V1 positioning changes, before hosted/dashboard work, and
  monthly while Allie is actively shaped.

## Children

1. Create the first sourced competitive matrix.
2. Add a landscape update checklist and freshness policy.
3. Backfill roadmap notes that state where Allie deliberately differs from each
   product category.
4. Add a lightweight validation script or test that flags unsourced or stale
   landscape rows.
5. Revisit the matrix after each dogfood run in Vanity, Linejam, Sploot, or
   Misty Step.

## Notes

**Why:** The competitive lane found Allie's strongest position is not rule-count
parity but defensible release evidence: packets, replay commands, artifact
graphs, provenance, non-blocking model governance, and standards drilldown. That
needs a living landscape artifact so product bets stay grounded.

