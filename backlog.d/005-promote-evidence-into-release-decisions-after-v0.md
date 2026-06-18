# Promote evidence into release decisions after V0

Priority: P2 - Status: pending - Estimate: XL

## Goal

After local replay is stable, connect Allie evidence packets to pull-request and
release decisions without letting model opinions, dashboards, or remediation
features outrun deterministic proof.

## Oracle

- [ ] PR/check output summarizes changed surfaces, required evidence, blocking
  deterministic/scripted failures, stale evidence, and review-needed obligations.
- [ ] Model enrichment remains policy-gated, audited, and non-blocking unless a
  human or deterministic rule promotes it.
- [ ] Waivers include provenance, expiry, touched-surface behavior, and packet
  references.
- [ ] Hosted/dashboard views read from the same evidence contract instead of a
  separate status model.

## Verification System

- Claim: release intelligence is a projection of evidence packets, not a second
  product truth source.
- Falsifier: PR status blocks on model-only findings, ignores packet verdicts,
  loses waiver provenance, or displays a global score detached from obligations.
- Driver: fixture packet set projected through PR/check and release-summary
  adapters.
- Grader: golden check payloads, report snapshots, and policy assertions.
- Evidence packet: fixture packet corpus plus generated PR/check payloads.
- Cadence: after V0 local packet/report are stable.

## Children

1. Define the release-decision summary model as a projection of evidence packets.
2. Add GitHub Checks payload generation for deterministic and scripted failures.
3. Add stale-evidence, missing-journey, and waiver-expiry logic.
4. Add model-gateway policy enforcement and audit records with calls disabled by
   default until explicit provider policy exists.
5. Shape hosted evidence viewer and trend ledger only after local packet
   semantics are proven.

## Notes

**Why:** Product/value and simplification lanes agreed that release visibility is
the durable product direction, but the first wedge must stay local and
deterministic. This epic preserves the ambition while sequencing PR, model,
waiver, remediation, and hosted surfaces behind stable evidence.

