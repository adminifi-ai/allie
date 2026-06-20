# Make WCAG coverage a 55-criterion surface matrix

Priority: P0 - Status: done - Estimate: XL

## Goal

Report every WCAG 2.2 A/AA success criterion exactly once per required product
surface/state, while separating Allie supporting checks and aggregate gates from
the WCAG denominator.

## Oracle

- [x] A `wcag22-aa` report has exactly 55 WCAG success-criterion rows and a
  separate supporting-check section.
- [x] Every discovered required surface/state has a criterion matrix with
  `status`, `applicability`, `method`, `confidence`, `evidence_refs`,
  `agentic_refs`, `waiver_refs`, and `residual_review_need`.
- [x] The Vanity dogfood report no longer counts
  `deterministic-axe-rules`, `*-keyboard-traversal`, `*-zoom-reflow`,
  `*-reduced-motion`, or human-review aggregates as extra WCAG standards.
- [x] Schema validation rejects a `pass`, `fail`, `waived`, or `risk_accepted`
  criterion cell without required provenance.
- [x] The HTML report can drill from criterion -> surface -> state -> finding ->
  artifact -> test/replay command -> agentic/human context.

## Verification System

- Claim: Allie grants complete standards visibility without overstating what was
  deterministically tested or inflating the WCAG count.
- Falsifier: a missing criterion, duplicate criterion, mixed supporting check in
  the WCAG denominator, surface with no matrix row, status without rationale, or
  model-only finding presented as compliant.
- Driver: fixture packets plus a checked-in Vanity dogfood packet fixture that
  reproduces the previous misleading `61` total.
- Grader: criterion-count tests, schema validation, golden JSON/HTML report
  snapshots, artifact-link checks, and no-legal-claim text assertions.
- Evidence packet: `.allie/runs/coverage-matrix-smoke/`,
  `.allie/reports/coverage-matrix-smoke/`, and fixture report snapshots.
- Cadence: before changing the profile schema, after denominator separation,
  after surface matrix generation, and before release projection consumes it.

## Children

1. Split `success_criteria`, `supporting_checks`, and `aggregate_checks` in the
   obligation profile and packet schema.
2. Introduce `criterion_coverage[]` keyed by
   `criterion_id + surface_id + state_id + policy_profile`.
3. Add applicability statuses and require rationale/provenance for
   `not_applicable`, `waived`, and `risk_accepted`.
4. Map supporting checks into one or more WCAG criteria without counting them as
   standards.
5. Render matrix-first HTML and JSON reports with drilldown evidence links.
6. Expand scripted coverage for reflow, keyboard traversal, reduced motion,
   focus visibility, focus not obscured, text spacing, target size, and status
   messages where automation is reliable.

## Notes

**Why:** The WCAG lane and Vanity dogfood showed the current ledger includes the
full WCAG 2.2 A/AA success-criterion list, then appends Allie supporting and
aggregate obligations as additional `wcag22-aa:*` verdicts. The next product
step is honest 55-row visibility across the discovered product surface.

## Delivered

- Added report-native `criteria`, `criterion_coverage`, and
  `supporting_checks`, with legacy `obligations` remaining criterion-only for
  compatibility.
- Added criterion/surface/state matrix validation for duplicate cells, missing
  cells, required fields, and terminal-status provenance.
- Added a checked-in Vanity legacy-61 fixture and `npm run coverage:smoke`,
  which proves support checks stay out of the WCAG denominator.
- Updated HTML and Markdown reports to show matrix drilldown, support checks,
  waiver/risk counts, and no-legal-guarantee language.
- Added regression tests for no unrelated axe evidence on `needs_review` and
  `not_tested` cells, waiver/risk-accepted cells from packet waivers, and
  multi-surface matrix completeness.
