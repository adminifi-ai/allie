# Pay down core architecture debt

Priority: P1 · Status: pending · Estimate: XL

## Goal
`src/lib.rs` stops being a god-module and the load-bearing boundaries (browser
worker adapter, standards taxonomy, typed packet path) are clean, so new
features land without growing one file.

## Oracle
- [ ] Browser worker spawn/IPC lives behind a `worker` adapter module; no `Command::new("node")` / `npx playwright` / `playwright-axe-worker` strings in lib.rs (src/lib.rs:2690,3400,3499 today — violates the AGENTS.md worker-adapter rule).
- [ ] Standards/WCAG taxonomy extracted to one module (de-dup the 16 criterion fns split across src/lib.rs:1836-4062 and src/compliance.rs:215-360).
- [ ] `run_cli_with_io` dispatch (src/lib.rs:256, 304 lines) split from per-command handlers; handlers unit-tested directly.
- [ ] Release/waiver path typed through model.rs DTOs (kill the `serde_json::Value` working-type sprawl, 36× in lib.rs:2410-2651).
- [ ] Size cap ratcheted from 6800 toward ~5200; `size:smoke` green.

## Children
1. Extract a `worker` adapter module (spawn/IPC); keep lib.rs evidence-packet-only. (do first — unblocks epic 015 child 1)
2. Extract a `standards`/`wcag` module owning all profile + criterion data.
3. Split `run_cli_with_io` dispatch from command handlers.
4. Type the release/waiver packet path.
5. Ratchet the size cap downward.

## Notes
**Why:** architecture lane. Operator priority (maintainability critical). lib.rs is 6426/6800 — ~374 lines of headroom, so one feature trips the gate. The worker-adapter extraction is also the clean seam epic 015's worker-action work needs, so sequence it first. `agentic.rs`/`model.rs`/`report.rs` are already cohesive leaves — leave them. Epic 017 (descope) removes ~hundreds of remediation lines from lib.rs ahead of this.
