# Make the vision invariants true and enforced

Priority: P1 · Status: pending · Estimate: M

## Goal
Allie's load-bearing invariants — replay/byte-stability, provenance, and
no-silent-gaps honesty — are actually true and gated by tests, not just claimed.

## Oracle
- [ ] Injectable clock (e.g. `SOURCE_DATE_EPOCH`/frozen) replaces unconditional `Utc::now()` (src/lib.rs:4596); a "render twice → byte-identical packet + report" gate exists and passes.
- [ ] A test asserts packet provenance `git_sha`/`git_branch` are non-empty and match `git rev-parse` (today `unwrap_or_default()` → `""`, src/lib.rs:3355).
- [ ] A test asserts a criterion with zero evidence renders `not_tested`, not `needs_review` (criterion_cell_status falls through to needs_review, src/compliance.rs:854-891).
- [ ] The live `evidence:smoke` path asserts content (confidence classes distinct, statuses explicit), not only file existence (scripts/verify.sh:17-49 is `test -f` checks).

## Children
1. Injectable clock + byte-identical replay gate.
2. Provenance non-empty + matches-revision assertion.
3. `not_tested` vs `needs_review` honesty test (uncovered cell must show as a gap).
4. Content assertions on the live evidence smoke.

## Notes
**Why:** testing/invariants lane. "Replayable evidence tied to a code revision" is Allie's strongest competitive moat (competitive lane) — but it is currently unprovable and likely false (`now_utc()` is wall-clock). The aggregate "zero not_tested" claim is true *by construction* (cells can't emit `not_tested`), which risks dressing a genuine gap as `needs_review` — directly at odds with the no-silent-gaps + honest-uncertainty invariants. The agentic-vs-deterministic distinction is already well-tested (report.rs:593) — preserve it.
