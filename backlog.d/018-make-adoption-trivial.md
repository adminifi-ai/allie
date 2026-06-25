# Make adoption trivial — clone into any repo and run

Priority: P1 · Status: pending · Estimate: L

## Goal
A developer clones Allie into any repo and runs it with near-zero setup — no
magic env var, a guided first run, and CI that does not reinstall the world.

## Oracle
- [ ] `ALLIE_BROWSER_WORKER` is no longer required; the worker auto-resolves/bundles relative to the installed binary (src/lib.rs:2676 uses build-time `CARGO_MANIFEST_DIR`, which breaks after `cargo install`).
- [ ] `allie doctor` preflight checks node / playwright / worker / target reachability with actionable fixes (today: raw `node` spawn error, src/lib.rs:2690-2705).
- [ ] `allie init` prints the full prerequisite + setup checklist as next steps (today: only "Next: allie verify", src/lib.rs:281).
- [ ] A prebuilt release artifact/container exists; CI adapters use it instead of per-run clone + `cargo install` (docs/ci/*.yml:14-30).
- [ ] README leads with a one-command consumer quickstart, not the contributor `cargo run` manual (README.md:90-237).

## Children
1. Bundle/auto-resolve the browser worker so `ALLIE_BROWSER_WORKER` is never needed. (THE lever — cascades to README, verification.md, both CI files)
2. `allie doctor` preflight with actionable fixes.
3. `allie init` emits the full prerequisite checklist.
4. Prebuilt release artifact/container + cached CI adapters.
5. Restructure README around the consumer quickstart.

## Notes
**Why:** adoption-friction lane. Operator priority (adoption critical). Single biggest lever = eliminate the env var by resolving the worker relative to the installed binary; it fixes README, verification.md, and both CI adapters at once. Once installed, the `init`/`verify` two-command contract is genuinely clean — the friction is all in getting installed.
