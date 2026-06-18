# Map standards obligations without legal claims

Priority: P1 - Status: pending - Estimate: L

## Goal

Map axe and scripted observations into versioned obligation profiles so Allie
can report standards-linked status, confidence, and residual review needs
without claiming legal compliance.

## Oracle

- [ ] `wcag22-aa` profile data can map axe rule tags and scripted checks to
  obligation-level verdicts.
- [ ] Verdicts distinguish `pass`, `fail`, `needs_review`, `not_tested`,
  `waived`, and `risk_accepted`.
- [ ] Confidence distinguishes `machine_proven`, `script_observed`,
  `agent_inferred`, and `human_attested`.
- [ ] The report names residual human-review obligations instead of collapsing
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

