# Raise WCAG report efficacy and quality

Priority: P1 · Status: pending · Estimate: XL

## Goal
Make Allie's WCAG report more judgeable, comparable, and useful by improving criterion traceability, report self-quality checks, advisory verdict calibration, and cross-run diffs.

## Oracle
- [ ] Report fixtures assert semantic content, not just that JSON/HTML/Markdown files exist.
- [ ] Each report clearly links criteria to surfaces, methods, artifacts, confidence, residual review, and release-policy effect.
- [ ] Dogfood refreshes can produce a compare view that explains what changed since the previous run.
- [ ] Judgment-heavy criteria are grouped, calibrated, and explained so the report does not degrade into a wall of `needs_review`.
- [ ] The report's own rendered accessibility, responsive behavior, and print/readability constraints are checked by an automated self-test.

## Verification System
- Claim: A developer, accessibility specialist, or downstream agent can use the report to understand the evidence and the residual review work without re-investigating the run.
- Falsifier: A report omits an in-scope criterion, cannot identify which surface produced a finding, obscures the method/confidence boundary, is itself inaccessible, or cannot explain a cross-run change.
- Driver: Structured report fixture tests plus a dogfood compare command over two saved evidence packets.
- Grader: Golden semantic assertions for JSON, HTML, Markdown, JUnit, and SARIF; browser checks for report layout and accessibility; manual review of one dogfood compare receipt.
- Evidence packet: `.allie/reports/<fixture>/`, `.allie/report-quality/<fixture>/`, and dogfood compare artifacts.
- Cadence: CI for fixtures; dogfood compare after every refreshed real-app receipt.

## Children
1. Add report-quality fixture tests that assert specific criterion, surface, artifact, confidence, and release-policy fields.
2. Add a report self-check that runs browser/a11y checks against generated report HTML at desktop and mobile widths.
3. Build an evidence/report diff that classifies findings as new, fixed, unchanged, unverified, or scope-changed.
4. Improve grouping for agentic and judgment-heavy WCAG criteria so residual review is scannable and not a flat list.
5. Strengthen JUnit and SARIF output tests so CI consumers receive the same semantics as the HTML report.
6. Add report readability guidance to docs without implying legal compliance.
7. Feed report-quality defects found by the dogfood ladder back into this epic.

## Notes
- The current gate checks many report files exist; this epic raises the bar to content and usefulness.
- Previous real-app receipts show high `needs_review` counts. That is honest uncertainty, but the report must make it operationally clear.
- Allie still does not remediate. The report is the handoff artifact, not a fixing queue.
