# Make adoption trivial — clone into any repo and run

Priority: P1 · Status: done · Estimate: L

## Goal
A developer clones Allie into any repo and runs it with near-zero setup — no
magic env var, a guided first run, and CI that does not reinstall the world.

## Oracle
- [x] `ALLIE_BROWSER_WORKER` is no longer required; the worker auto-resolves/bundles relative to the installed binary (src/lib.rs:2676 uses build-time `CARGO_MANIFEST_DIR`, which breaks after `cargo install`).
- [x] `allie doctor` preflight checks node / playwright / worker / target reachability with actionable fixes (today: raw `node` spawn error, src/lib.rs:2690-2705).
- [x] `allie init` prints the full prerequisite + setup checklist as next steps (today: only "Next: allie verify", src/lib.rs:281).
- [x] A prebuilt release artifact/container exists; CI adapters use it instead of per-run clone + `cargo install` (docs/ci/*.yml:14-30).
- [x] README leads with a one-command consumer quickstart, not the contributor `cargo run` manual (README.md:90-237).

## Children
1. Bundle/auto-resolve the browser worker so `ALLIE_BROWSER_WORKER` is never needed. (THE lever — cascades to README, verification.md, both CI files)
2. `allie doctor` preflight with actionable fixes.
3. `allie init` emits the full prerequisite checklist.
4. Prebuilt release artifact/container + cached CI adapters.
5. Restructure README around the consumer quickstart.

## Notes
**Why:** adoption-friction lane. Operator priority (adoption critical). Single biggest lever = eliminate the env var by resolving the worker relative to the installed binary; it fixes README, verification.md, and both CI adapters at once. Once installed, the `init`/`verify` two-command contract is genuinely clean — the friction is all in getting installed.

## Closure
- Added `allie doctor` for worker, Node, Playwright, and target preflight, with
  a durable `.allie/doctor/doctor.json` receipt.
- Browser worker resolution no longer requires `ALLIE_BROWSER_WORKER` on the normal consumer path; it supports bundled distribution root, installed lib/share roots, Cargo target checkout, and explicit env override.
- Added release bundle packaging plus tag-publish workflow, and changed CI examples to consume the bundle instead of cloning/building Allie.
- Added `distribution:smoke` and wired it into `npm run verify`; full gate passed with the bundled binary capturing evidence from a foreign consumer repo.
