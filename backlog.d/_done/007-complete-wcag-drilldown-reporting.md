# Build complete WCAG obligation ledger and drilldown reporting

Priority: P0 - Status: done - Estimate: XL

## PRD Summary

- User: accessibility compliance engineers and audit stakeholders.
- Problem: Allie currently maps a small set of axe tags plus placeholder
  scripted/human obligations; it cannot show every WCAG requirement, why a
  requirement is compliant or noncompliant, or which evidence supports that
  status.
- Goal: make WCAG 2.2 A/AA a complete obligation ledger and make the report drill
  from standard to criterion to flow state to test, artifact, context, waiver,
  and remediation.
- Why now: comprehensive reporting is the product spine; generated tests and
  agentic review need a criterion-level destination.
- UX enabled: a compliance engineer can open the report, choose any WCAG
  criterion, and see status, rationale, evidence, gaps, and next action.
- Deliverable type: data contract, report UX, and tests.
- Success signal: every WCAG 2.2 A/AA criterion has an explicit status and method
  record in generated evidence.

## Product Requirements

- P0: replace the scaffold profile with a complete WCAG 2.2 A/AA obligation
  ledger: principle, guideline, success criterion, level, normative source,
  applicability, automation method, scripted method, human/agentic review method,
  and residual review need.
- P0: type `findings`, `verdicts`, `coverage`, and `review` in the formal JSON
  Schema instead of accepting loose arrays/objects.
- P0: report each criterion as `pass`, `fail`, `not_applicable`, `not_tested`, or
  `needs_review`, with rationale and confidence.
- P0: drilldown from criterion to affected state, selector/node refs, axe rule,
  Playwright assertion, screenshot/video/GIF, DOM/a11y tree, model review attempt,
  waiver, and remediation guidance when present.
- P0: preserve no-score and no-legal-guarantee language.
- P1: provide separate report views for engineer fix list, accessibility
  specialist review queue, and audit summary.
- Non-goals: live provider calls, automatic legal certification, or replacing
  expert review.

## Technical Design

- Keep the evidence packet as the core interface and add a compatible v1 shape if
  v0 cannot be tightened without breaking existing receipts.
- Use W3C's WCAG JSON serialization or an audited generated data file as the
  source for the obligation ledger.
- Use axe tags as one evidence source, not as the coverage denominator.
- Add report tests against fixture packets for pass/fail/not-applicable/not-tested
  and needs-review cases.

## Lead Repo Read

- `profiles/wcag22-aa.json`: currently a partial scaffold.
- `schemas/allie.evidence.v0.schema.json`: loose schema for key report fields.
- `src/lib.rs`: obligation mapping and report rendering.
- `docs/evidence-contract.md`: packet contract and release projection semantics.
- `docs/roadmap.md`: richer capture and reporting sequencing.

## Deliverable

- Output: complete obligation profile, typed schema, report drilldown, fixture
  packet corpus, and release-summary coverage fields.
- Acceptance oracle: fixture evidence produces a local report where every WCAG
  2.2 A/AA success criterion appears exactly once with status, method, rationale,
  evidence refs, and residual review state.
- Evidence artifacts: report snapshots, schema validation logs, generated
  obligation ledger checksum, and packet fixtures.
- Residual risk: conformance remains a professional judgment; Allie reports
  evidence and gaps, not legal compliance.

## Verification System

- Claim: Allie can make every WCAG requirement visible and inspectable.
- Falsifier: missing criterion, duplicate criterion, unsupported global score,
  unmapped axe finding, untyped finding/verdict, broken artifact link, or
  not-tested obligation presented as compliant.
- Driver: generated ledger tests plus fixture packet report rendering.
- Grader: schema validation, criterion-count assertions, golden report snapshots,
  and no-legal-claim text checks.
- Evidence packet: fixture packets and HTML report snapshots.
- Cadence: before profile generation, after schema tightening, after report
  drilldown, and before merge.

## Children

1. Add complete WCAG 2.2 A/AA source ingestion and checksum.
2. Define obligation, method, finding, verdict, coverage, and review schemas.
3. Map axe tags and scripted assertions into criterion-level evidence.
4. Render criterion drilldown report with artifact and context links.
5. Add fixture packet corpus covering each status class.
6. Add release-summary coverage denominators and gap accounting.

## Notes

W3C describes WCAG 2.2 as 13 guidelines under four principles, with testable
success criteria at A, AA, and AAA levels. Allie should anchor reporting on those
success criteria and use ACT-style transparent test methods for automated,
semi-automated, and manual checks.

## Delivered

- Expanded `profiles/wcag22-aa.json` with the full WCAG 2.2 A/AA success
  criterion ledger from W3C's machine-readable WCAG 2.2 JSON, excluding obsolete
  `4.1.1`.
- Tightened `schemas/allie.evidence.v0.schema.json` for typed run, policy,
  coverage, findings, verdicts, and review attempts.
- Report verdicts now show criterion titles, statuses, confidence, evidence
  class, and source.
- Coverage and verdict generation now include the full criterion denominator,
  while preserving custom scripted and human review obligations.
- Verified by `cargo test --locked` criterion-count assertions and
  `npm run autonomous:smoke` report/packet checks.
