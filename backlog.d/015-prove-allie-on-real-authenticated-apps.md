# Prove Allie on a real authenticated app

Priority: P0 · Status: pending · Estimate: XL

## Goal
Allie points at a real, running, authenticated web app — not the checked-in
fixture — logs in, discovers its surface, and produces a trustworthy WCAG audit
packet.

## Oracle
- [ ] Worker executes pre-state action steps (fill/click/type/waitFor), not just `goto`.
- [ ] BYO credentials reach the browser (env → login step → reused storageState); creds never written to packets (redaction holds).
- [ ] Discovery crawls a live `base_url` target (HTTP fetch + link graph), not only the fixture filesystem.
- [ ] Live agentic review runs in the workbench loop (no `offline-recorded`/`allie-vision-fixture` hardcode) when `model.enabled`.
- [ ] `verify` flags/refuses unauthenticated coverage when an auth profile implies a session (no false "no violations").
- [ ] Dogfood receipt: a real authenticated app from a misty-step / adminifi-ai / personal repo audited end-to-end, receipt preserved.

## Verification System
- Claim: Allie audits a real authenticated app and the packet reflects authenticated surfaces.
- Falsifier: run against a login-gated app; fails if the packet only holds the login page / unauthenticated routes, or auth is "set but unused".
- Driver: `allie verify` against a staged real app with a credential profile.
- Grader: packet shows authenticated surfaces + axe/agentic results; manual spot-check that gated content was reached.
- Evidence packet: `.allie/verify/<app>/` (map, evidence, report, release); dogfood receipt committed under `docs/dogfood/`.
- Cadence: per-change on the auth/crawl/worker path; dogfood receipt refreshed each milestone.

## Children
1. Worker action steps: fill/click/type/waitFor in `ManifestState` (src/lib.rs:3007) + `workers/browser/run.mjs:128`. (prereq for all)
2. Credential injection: env-sourced creds → login step → storageState reuse; redaction preserved (`WorkerRequest` src/lib.rs:3029).
3. Live authenticated crawl/discovery for `base_url` targets (HTTP, link graph, sitemap, route budget) — replaces fixture-only `discover_surfaces` (src/lib.rs:1915).
4. Wire live agentic review into the workbench loop — remove offline-fixture hardcode (`workbench.rs:390` → `lib.rs:2021-2150`).
5. Honest-coverage guardrail: `verify` flags unauthenticated/partial coverage instead of implying pass.
6. Changed-surface inference from git/route diff against discovery (replaces operator-supplied list, src/lib.rs:603).

## Notes
**Why:** real-app-proving lane (auth preflight-only `lib.rs:2862`; worker goto-only `run.mjs:128`; no credential serialization `lib.rs:3029`) + autonomy-depth lane (discovery fixture-only `lib.rs:1915`; workbench review offline `workbench.rs:390`). Operator priority #1. Land the worker-adapter extraction (epic 019, child 1) first to give child 1 here a clean seam.
