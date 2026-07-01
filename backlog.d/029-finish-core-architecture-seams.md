# Finish core architecture seams

Priority: P1 · Status: pending · Estimate: XL

## Goal
Complete the behavior-preserving extraction of Allie's core seams so `src/lib.rs` stops being the default home for manifests, CLI parsing, worker conversion, verdict policy, report rendering, and packet assembly.

## Oracle
- [ ] `src/lib.rs` is a thin crate hub rather than the largest feature module.
- [ ] Manifest DTOs, CLI option parsing, worker-response conversion, verdict/status policy, report rendering, and packet assembly each live in their owning module.
- [ ] Stringly verdict/status/confidence comparisons are replaced with typed enums at deterministic boundaries.
- [ ] The module-size gate is lowered after extraction and catches future backslide.
- [ ] `cargo fmt --check`, `cargo test --locked`, `cargo clippy --locked -- -D warnings`, and `npm run verify` pass with no schema or product behavior changes.

## Verification System
- Claim: The refactor reduces coupling and improves type safety without changing Allie's evidence packet, manifest, report, or CLI behavior.
- Falsifier: Any existing smoke output changes unintentionally, any public command changes shape, typed verdicts obscure the evidence-class boundary, or new shallow pass-through modules appear.
- Driver: Type-directed extraction in small commits plus the full repo gate.
- Grader: Diff review for ownership boundaries, snapshot/semantic smoke comparison, lowered size cap, and no behavior deltas in existing artifacts except path/module internals.
- Evidence packet: Standard `.allie/` smoke artifacts plus architecture notes in the delivery receipt.
- Cadence: Sequence before adding major new autonomous/report/release features that would otherwise grow `lib.rs`.

## Children
1. Move `FlowManifest` and related manifest DTOs into the shared model layer or a focused manifest module.
2. Move CLI option parsing and command dispatch ownership into `src/cli.rs`, keeping command behavior unchanged.
3. Move worker-response-to-finding conversion into the worker/evidence boundary.
4. Move verdict/status/confidence policy into typed enums and the compliance/release modules that own the decisions.
5. Move run-summary rendering into the report module and consolidate shared report styling decisions.
6. Extract packet assembly only if its imports show a clean deep-module boundary; otherwise leave it as the crate kernel and document why.
7. Lower the module-size gate and update `docs/roadmap.md` code-health notes after the extraction lands.

## Notes
- The old epic 019 reduced `src/lib.rs` dramatically, but it is still 4,700+ lines and remains the largest accumulation point.
- The roadmap already names typed verdicts and more decomposition as code-health work.
- This is not a rewrite and not a CLI-framework migration. Move cohesive code to existing homes first.
