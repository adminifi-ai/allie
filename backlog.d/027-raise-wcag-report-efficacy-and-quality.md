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

## Dogfood findings (2026-07-01, Vanity vs Olympus)
Source: `docs/dogfood/025-vanity-vs-olympus-cross-target.md`. The dogfood ladder surfaced concrete report-efficacy defects:
- R1 (HIGH): With the model off, the report degrades to a wall of `needs_review`/`not_tested`. Model-enabled Vanity: pass 39 / needs_review 2 / not_tested 0. Model-disabled Olympus (ZDR): pass 0 / needs_review 44 / not_tested 8. Judgment-heavy criteria need deterministic heuristics, grouping, and calibration that work with the model **off** (child 4, scoped to the model-off path).
- R2 (HIGH): Headline `not_tested` can hide obligation-grain gaps. Vanity's 55-criterion headline shows `not_tested: 0`, but the 60-obligation coverage ledger lists 4 untested (reflow, zoom-reflow, keyboard-traversal, reduced-motion) that roll up into passing criteria — a "no silent gaps" (VISION) risk. Reconcile obligation vs success-criterion grain (children 1, 4).
- R3: At least three unreconciled "review" counts for one run — needs_review 2, review-needed 7 (printed in the same `allie-report.md`), requiring-human-review 46. Add one reconciled review view (child 4).
- R4: Headline counts are not a safe cross-run diff signal — two identical Vanity runs gave pass 37 vs 39 / needs_review 4 vs 2 from agentic drift alone. The diff must classify deterministic vs agentic-advisory changes (child 3).
- R5: `allie verify` does not clean/namespace `--out`; stale artifacts from a prior run (an obsolete `remediation/` stage the tool no longer emits) persisted and could appear to contradict the no-remediation invariant. Emit a per-run file manifest or clean/version the out-dir so the packet dir describes exactly one run.
- R6 (minor): Manifests under-declare `known_nondeterminism` (`[]`) despite demonstrable agentic variance; fold into report-quality checks.

## Groom findings (2026-07-08, mega-sweep)
Board of record: Habitat (this epic = AL-003). Plan: `docs/plans/032-mega-groom-execution.html`.
- R3 is half-fixed: AL-090 unified review-count *computation*, but `src/consumer.rs:547,726,732` still prints unlabeled, indistinguishable review numbers → **AL-123**.
- R5 verified still unfixed and ungated (all write sites `create_dir_all`, never clean) → **AL-117**.
- R4 root fix carded: split deterministic vs agentic-advisory counts on every summary surface → **AL-124** (also answers the Codex premise review's verdict-blending objection without abandoning committed verdicts).
- HTML report omits the `replay_command`/`evidence_refs`/`artifact_refs` the JSON already carries (report.rs:248-282, 376-394) — reinforces AL-093; SARIF emits one synthetic rollup result total (consumer.rs:839-884) → **AL-121**; findings lack selectors — reinforces AL-092.
- Child 4 scope note: grouping/calibration (presentation) is a different swimlane from *reducing* needs_review via probes — the probe floor lives in AL-066/AL-116 (model-off 11/55 → ~16/55), sequenced independently.
