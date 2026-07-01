# Ratchet quality gates and performance proof

Priority: P1 · Status: pending · Estimate: XL

## Goal
Make Allie's quality floor more semantic, faster to diagnose, performance-aware, and harder to accidentally weaken.

## Oracle
- [ ] `npm run verify` reports timing by gate and identifies the slowest steps.
- [ ] Existing smoke scripts assert meaningful content for their artifacts instead of relying mostly on file existence.
- [ ] The module-size ratchet covers all Rust source under `src/`, not only top-level `src/*.rs`, and has a documented lower-ratchet path.
- [ ] Performance benchmarks cover at least worker IPC, evidence serialization, compliance matrix/report generation, and release projection.
- [ ] CI distinguishes fast local diagnostics from the full release-grade gate without weakening the latter.

## Verification System
- Claim: The gate catches likely regressions in behavior, report semantics, architecture, and performance while staying practical for daily use.
- Falsifier: A malformed or semantically wrong artifact can pass because the file exists, a large source file escapes the size gate, a major runtime regression has no signal, or the fast/full split diverges in policy.
- Driver: `npm run verify`, semantic smoke scripts, module-size gate, and a benchmark or timing command.
- Grader: Failing-fixture tests, JSON assertions, gate timing summary, module-size coverage over the full tree, and reviewed benchmark thresholds.
- Evidence packet: `.allie/gates/latest/` or equivalent timing/assertion output plus CI logs.
- Cadence: Every change runs the full repo gate before completion; performance trend checks run before large worker/report/autonomy changes.

## Children
1. Add a gate timing wrapper that records each `scripts/verify.sh` step and prints a ranked summary.
2. Replace existence-only artifact checks with structured JSON assertions for evidence, report, release, consumer, agentic, and autonomous smokes.
3. Expand `scripts/module-size-gate.sh` to scan all relevant Rust source files and ratchet the cap after architecture extraction.
4. Add performance benchmarks or stable timing probes for the high-cost local paths.
5. Add a known-bad fixture that must fail with a specific finding and a known-good fixture that must not block.
6. Split documented local-fast and full-release gates while keeping `npm run verify` as the canonical completion command.
7. Add guardrails that make gate-threshold changes explicit and reviewable.

## Notes
- `scripts/verify.sh` is broad and valuable; the next quality step is semantic depth, not fewer checks.
- `scripts/module-size-gate.sh` currently says "all files under src/" but only loops over `src/*.rs`.
- Do not lower gates or replace deterministic checks with model-only assurances.
