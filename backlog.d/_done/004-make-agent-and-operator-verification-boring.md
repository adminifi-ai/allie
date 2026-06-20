# Make agent and operator verification boring

Priority: P1 - Status: done - Estimate: M

## Goal

Give a cold agent or human operator one documented verification path that builds,
tests, runs the V0 smoke, and preserves inspectable evidence without relying on
chat context.

## Oracle

- [x] A repo-local verification runbook names the exact commands, expected
  artifacts, failure meanings, and cleanup rules.
- [x] CI runs `cargo fmt --check`, `cargo test --locked`, and the V0 smoke once
  the browser worker exists.
- [x] The smoke leaves a stable evidence packet path suitable for review.
- [x] `AGENTS.md` and `README.md` remain consistent with the actual gates.

## Verification System

- Claim: a fresh checkout can verify Allie without hidden operator knowledge.
- Falsifier: following the runbook misses a required gate, leaves unexplained
  artifacts, cannot run the smoke, or produces outputs that no reviewer can
  inspect.
- Driver: fresh-checkout command sequence plus CI workflow.
- Grader: command exits, artifact presence, README/AGENTS consistency, and final
  clean-tree check.
- Evidence packet: V0 smoke output and CI logs.
- Cadence: after the V0 smoke exists, then every closeout.

## Children

1. Add a verification runbook for local build, test, V0 smoke, evidence review,
   and cleanup.
2. Add a CI workflow for Rust gates.
3. Extend CI with the browser-worker smoke after the worker lands.
4. Add a repo-local receipt/evidence convention for `.allie/runs/`.
5. Keep AGENTS and README gate instructions synchronized with implementation.

## Notes

**Why:** Verification and agent-readiness lanes found only generic Rust gates
today. The repo has no `.github` workflow, no fixture, no smoke command, and no
runbook for interpreting evidence artifacts.

## Delivered

- Added `docs/verification.md` as the cold operator runbook with setup, gate, expected artifacts, failure meanings, trust-boundary fixtures, release projection, and cleanup.
- Added `scripts/verify.sh` and `npm run verify` as the single local verification path.
- Added CI workflow `.github/workflows/ci.yml` for Rust gates, browser worker smoke, V0 evidence smoke, and release projection smoke.
- Updated `AGENTS.md` and `README.md` so the documented gate matches the implemented commands.
- Verified with `npm run verify`, which produced `.allie/runs/v0-smoke/evidence.json`, `.allie/runs/v0-smoke/report.html`, `.allie/releases/v0-smoke/release-summary.json`, `.allie/releases/v0-smoke/github-check.json`, and `.allie/releases/v0-smoke/release-report.html`.
