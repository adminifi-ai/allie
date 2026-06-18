# Make agent and operator verification boring

Priority: P1 - Status: pending - Estimate: M

## Goal

Give a cold agent or human operator one documented verification path that builds,
tests, runs the V0 smoke, and preserves inspectable evidence without relying on
chat context.

## Oracle

- [ ] A repo-local verification runbook names the exact commands, expected
  artifacts, failure meanings, and cleanup rules.
- [ ] CI runs `cargo fmt --check`, `cargo test --locked`, and the V0 smoke once
  the browser worker exists.
- [ ] The smoke leaves a stable evidence packet path suitable for review.
- [ ] `AGENTS.md` and `README.md` remain consistent with the actual gates.

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

