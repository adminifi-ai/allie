# Map standards obligations without legal claims

Priority: P1 - Status: done - Estimate: L

## Goal

Map axe and scripted observations into versioned obligation profiles so Allie
can report standards-linked status, confidence, and residual review needs
without claiming legal compliance.

## Oracle

- [x] `wcag22-aa` profile data can map axe rule tags and scripted checks to
  obligation-level verdicts.
- [x] Verdicts distinguish `pass`, `fail`, `needs_review`, `not_tested`,
  `waived`, and `risk_accepted`.
- [x] Confidence distinguishes `machine_proven`, `script_observed`,
  `agent_inferred`, and `human_attested`.
- [x] The report names residual human-review obligations instead of collapsing
  them into a score.

## Verification System

- Claim: standards mapping is transparent, versioned, and narrower than legal
  compliance.
- Falsifier: a profile reports a global score, marks untested obligations as
  pass, loses confidence/provenance, or presents ADA/Section 508 status as a
  legal guarantee.
- Driver: fixture axe outputs and scripted observations mapped through the
  `wcag22-aa` profile.
- Grader: golden verdicts and report text assertions.
- Evidence packet: standards-mapping fixture packets and report snapshots.
- Cadence: after the V0 packet schema exists and before expanding policy packs.

## Children

1. Define the obligation-profile data shape for `wcag22-aa`.
2. Map initial axe WCAG rule tags to obligations with source references.
3. Add scripted-check placeholders for keyboard, focus, zoom/reflow, and reduced
   motion states.
4. Preserve `needs_review` and `not_tested` obligations in packet and report.
5. Add report copy that explicitly avoids legal compliance claims.

## Notes

**Why:** The product contract says Allie models standards as obligation
profiles, not a score, and never claims legal compliance. External primary
sources support this shape: W3C WCAG 2.2 defines the recommendation, W3C ACT
documents transparent automated/semi-automated/manual test rules, and axe-core
rules carry WCAG version and success-criterion tags.

## Delivered

- Added `profiles/wcag22-aa.json` with axe-tag mapping, deterministic pass, scripted placeholder, and human-review obligations.
- Mapped axe findings and passing deterministic evidence into obligation-level verdicts with confidence/provenance.
- Added `not_tested` scripted obligations and `needs_review` human obligations to packets, coverage, and reports.
- Kept report language explicit that evidence is not a legal compliance guarantee and does not produce a compliance score.
- Verified with `cargo test --locked`, `npm run verify`, and live `.allie/runs/latest/evidence.json` showing one deterministic pass, three `not_tested` obligations, and two `needs_review` obligations.
