# Add remediation and release enforcement workbench

Priority: P2 - Status: done - Estimate: XL

## PRD Summary

- User: compliance engineers and frontend teams closing accessibility findings.
- Problem: Allie can emit suggested remediation text in findings, but it does not
  manage evidence-linked remediation queues, branch-scoped patch attempts,
  reviewer attestations, or enforcement workflows.
- Goal: turn assessment evidence into bounded remediation and release decisions
  without auto-merging or weakening gates.
- Why now: autonomous assessment is not valuable enough unless it drives fixes and
  prevents regressions.
- UX enabled: teams can drill from release blocker to source hint, patch plan,
  replay command, waiver context, and reviewer decision.
- Deliverable type: workbench report, CLI workflow, and release adapters.
- Success signal: every remediation action is causally linked to findings,
  artifacts, replay proof, and promotion state.

## Product Requirements

- P0: create an evidence-linked remediation queue for blocking deterministic and
  scripted findings.
- P0: include likely source files/components, suggested fix, confidence, artifact
  refs, affected WCAG criteria, and replay command for each item.
- P0: allow branch-scoped patch drafting only when there is finding evidence and
  a replay command.
- P0: rerun relevant evidence after a patch and compare before/after packets.
- P0: support waivers and reviewer attestations with provenance, expiry, touched
  surface behavior, and packet refs.
- P0: emit GitHub Checks and PR comment payloads from evidence packets.
- P1: add report views for fix list, review queue, waiver ledger, and release
  summary.
- Non-goals: auto-merge, legal certification, model-only blocking, or broad code
  rewrites without evidence.

## Technical Design

- Rust owns remediation item schema, action/audit ledger, release projection,
  waiver validation, and before/after comparison.
- Patch generation, if added, runs as a bounded adapter with explicit write scope
  and never bypasses replay verification.
- All reports and PR/check payloads remain projections from packets and action
  ledgers.

## Lead Repo Read

- `SPEC.md`: remediation model and release policy.
- `src/lib.rs`: current finding remediation field and release projection.
- `docs/evidence-contract.md`: waiver and release projection semantics.
- `docs/roadmap.md`: workbench and PR integration sequencing.

## Deliverable

- Output: remediation queue schema, action ledger, before/after packet comparison,
  workbench report views, GitHub payloads, and patch-attempt guardrails.
- Acceptance oracle: a known fixture failure produces a remediation item, a patch
  attempt is refused without evidence/replay, accepted patch attempts rerun the
  relevant flow, and release projection reflects before/after status.
- Evidence artifacts: remediation queue, action ledger, before/after packets,
  reports, GitHub payloads, and replay transcripts.
- Residual risk: source hints may be probabilistic and must remain confidence
  scored until verified by tests.

## Verification System

- Claim: remediation is evidence-locked and enforceable without unsafe autonomy.
- Falsifier: patch draft without finding evidence; missing replay command;
  source hint treated as certain; waiver without expiry/provenance; model-only
  finding blocks release; or GitHub payload diverges from packet truth.
- Driver: known fixture failures and before/after patch fixtures.
- Grader: schema validation, release projection tests, report snapshots, patch
  guard tests, and waiver tests.
- Evidence packet: before/after evidence packets plus remediation action ledger.
- Cadence: after discovery, WCAG drilldown, generated tests, and agentic review
  contracts are available.

## Children

1. Define remediation item and action ledger schemas.
2. Add before/after packet comparison.
3. Render remediation workbench views from packet data.
4. Add guarded branch-scoped patch drafting contract.
5. Add GitHub Checks and PR comment projection from packets.
6. Add waiver and reviewer-attestation workflows.

## Notes

Remediation is where Allie becomes operationally useful. It should still obey the
core rule: no action without evidence, replay, provenance, and an explicit
release-policy effect.

## Delivered

- Added `allie remediate --packet <evidence.json> --out <dir>`.
- Added `allie.remediation-queue.v0`, `allie.action-ledger.v0`,
  `remediation-report.html`, and `patch-plan.md` outputs.
- Remediation items include finding refs, WCAG obligation, affected state,
  artifact refs, source hints, suggested fix, confidence, replay command, and
  policy effect.
- Release projection continues to block deterministic/scripted packet failures
  and missing evidence, while keeping model-only context neutral.
- Verified by `npm run autonomous:smoke`, which writes remediation receipts and
  blocks the reviewed packet on the known deterministic contrast failure.
