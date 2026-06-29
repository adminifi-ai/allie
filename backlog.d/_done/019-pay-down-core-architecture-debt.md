# Pay down core architecture debt

Priority: P1 · Status: done · Estimate: XL

## Goal
`src/lib.rs` stops being a god-module and the load-bearing boundaries (browser
worker adapter, standards taxonomy, typed packet path) are clean, so new
features land without growing one file.

## Oracle
- [x] Browser worker spawn/IPC lives behind a `worker` adapter module; no `Command::new("node")` / `npx playwright` / `playwright-axe-worker` strings in lib.rs (src/lib.rs:2690,3400,3499 today — violates the AGENTS.md worker-adapter rule).
- [x] Standards/WCAG taxonomy extracted to one module (de-dup the 16 criterion fns split across src/lib.rs:1836-4062 and src/compliance.rs:215-360).
- [x] `run_cli_with_io` dispatch (src/lib.rs:256, 304 lines) split from per-command handlers; handlers unit-tested directly.
- [x] Release/waiver path typed through model.rs DTOs (kill the `serde_json::Value` working-type sprawl, 36× in lib.rs:2410-2651).
- [x] Size cap ratcheted from 6800 toward ~5200; `size:smoke` green.

## Children
1. Extract a `worker` adapter module (spawn/IPC); keep lib.rs evidence-packet-only. (do first — unblocks epic 015 child 1)
2. Extract a `standards`/`wcag` module owning all profile + criterion data.
3. Split `run_cli_with_io` dispatch from command handlers.
4. Type the release/waiver packet path.
5. Ratchet the size cap downward.

## Notes
**Why:** architecture lane. Operator priority (maintainability critical). lib.rs is 6426/6800 — ~374 lines of headroom, so one feature trips the gate. The worker-adapter extraction is also the clean seam epic 015's worker-action work needs, so sequence it first. `agentic.rs`/`model.rs`/`report.rs` are already cohesive leaves — leave them. Epic 017 (descope) removes ~hundreds of remediation lines from lib.rs ahead of this.

## Delivered
- Added `src/worker.rs` for browser-worker request/response DTOs, auth-safe request construction, spawn/timeout handling, and artifact metadata.
- Added `src/standards.rs` for WCAG 2.2 AA profile helpers, obligation mapping, criterion metadata, and feature-to-verdict rules.
- Added `src/release.rs` plus typed release DTOs in `src/model.rs` for release projection, stale evidence checks, waiver validation, GitHub check payloads, and release report rendering.
- Added `src/cli.rs` for CLI dispatch and command handlers, with direct handler tests for usage errors.
- Ratcheted `scripts/module-size-gate.sh` from 6800 to 5200 lines; `src/lib.rs` is now below the cap.

## Verification
- `cargo check --locked`
- `cargo test --locked release_projection -- --nocapture`
- `cargo test --locked worker_request_carries_auth_env_names_not_secret_values -- --nocapture`
- `cargo test --locked cli::tests -- --nocapture`
- Structural ownership greps for worker and standards helpers in `src/lib.rs`
- `npm run verify`
