# Prove Allie across the real-app dogfood ladder

Priority: P0 · Status: in-progress · Estimate: XL

## Goal
Turn Vanity, Olympus, Habitat, and Linejam into a recurring real-app evidence ladder that proves Allie can produce comparable surface maps, browser evidence, WCAG reports, and release projections across public, authenticated, operational, and highly interactive apps.

## Oracle
- [ ] Each target has a committed dogfood receipt under `docs/dogfood/<target>/` with target checkout, git state, auth model, manifest path, command, exit interpretation, and artifact inventory.
- [ ] The ladder includes at least one static/public app, one authenticated consumer app, one work-management app, and one operations/control-plane app.
- [ ] Every receipt distinguishes Allie infrastructure/auth failures from real target accessibility findings.
- [ ] Dirty or user-owned target repos are not modified by the dogfood run; their status is recorded before and after.
- [ ] A cross-target summary identifies mapping misses, generated-flow misses, report-quality defects, and auth friction that should become Allie work.

## Verification System
- Claim: Allie can run the composed evidence loop against representative real apps and preserve comparable, replayable receipts without mutating the target repositories.
- Falsifier: A target cannot produce a packet for reasons Allie should handle, receipts are not comparable, auth loss is misreported as app evidence, or a dogfood run changes target repo state.
- Driver: `allie verify --manifest <manifest> --out .allie/dogfood/<target> --project-root <target-checkout>` plus target-specific local start commands documented in each receipt.
- Grader: Receipt matrix with route/state count, infra failures, auth state, deterministic findings, WCAG summary, release status, artifact paths, and before/after `git status`.
- Evidence packet: `.allie/dogfood/<target>/` for generated artifacts and `docs/dogfood/<target>/` for committed receipts.
- Cadence: Refresh the ladder after each major mapping, report, auth, worker, or release-policy change.

## Children
1. Inventory the four target checkouts and write committed manifests/receipts for their current git state, start commands, auth model, and safe operating boundaries.
2. Run an Olympus unauthenticated smoke against `/dashboard/login` to prove Allie captures a Hono-rendered dashboard surface before auth complicates the signal.
3. Add an Olympus cookie-auth run using `DASHBOARD_SECRET` by env name only, then target `/health`, dashboard, observability, agent/job, recorder, and metrics surfaces without production mutation.
4. Refresh the clean Vanity static/public run and compare it to the existing `docs/dogfood/014-vanity-consumer-contract.md` receipt.
5. Refresh Linejam authenticated coverage only when its dirty worktree is explicitly safe; regenerate Clerk storageState through the test-key path and compare with `docs/dogfood/linejam/015-authenticated-verify.md`.
6. Add a Habitat run starting with safe public/unauthenticated surfaces, then add Supabase storageState or an operator-provided session for gated routes.
7. Add a Linejam game-state slice that measures whether Allie can exercise in-page states beyond URL discovery.
8. Create a cross-target dogfood summary that converts repeat failures into child tickets or updates to epics 026 and 027.
9. Add a lightweight script or Make target that reruns the safe subset of the ladder and refuses to run against dirty targets unless explicitly allowed.

## Notes
- Vanity is currently a clean static checkout at `/Users/phaedrus/Development/vanity`.
- Olympus is currently a clean Adminifi control-plane checkout at `/Users/phaedrus/Development/adminifi/olympus`.
- Olympus is the best first substantive slice: it is clean, data-dense, cookie-authenticated, and has real dashboard/control-plane surfaces.
- Linejam is currently dirty in `/Users/phaedrus/Development/linejam`; treat all changes as user-owned.
- Habitat was observed changing during this groom and ended on branch `refactor/ha-004-break-docs-bot-entanglements` with remote divergence; re-check live status before dogfood and treat all non-Allie changes as user-owned.
- The point is not a pass/fail vanity metric. The product claim is that Allie can explain exactly what it saw, what failed, what was unverified, and what evidence backs the release decision.

## Receipts
- 2026-07-01: Olympus dashboard slice delivered in `docs/dogfood/olympus/025-olympus-dashboard-dogfood.md`. Allie captured the login page plus six authenticated dashboard states with zero infrastructure failures in the authenticated verify run; the release projection blocked on deterministic target findings.
