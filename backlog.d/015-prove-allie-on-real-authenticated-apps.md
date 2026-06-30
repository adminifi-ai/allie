# Prove Allie on a real authenticated app

Priority: P0 · Status: pending · Estimate: XL

## Goal
Allie points at a real, running, authenticated web app — not the checked-in
fixture — logs in, discovers its surface, and produces a trustworthy WCAG audit
packet.

## Oracle
- [x] Worker executes pre-state action steps (fill/click/type/waitFor), not just `goto`.
- [ ] BYO credentials reach the browser (env → login step → reused storageState); creds never written to packets (redaction holds).
- [x] Discovery crawls a live `base_url` target (HTTP fetch + link graph), not only the fixture filesystem.
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
**Shaped:** Slice 1 — children 1 (worker action steps), 2 (credential injection),
and 5 (honest-coverage guardrail) — is `/deliver`-ready as ticket
[023](023-authenticated-audit-on-real-apps-slice-1.md): log in to a real app and
audit hand-listed authenticated routes. Children 3 (live crawl), 4 (wire live
agentic review), and 6 (changed-surface from git) stay deferred to follow-on slices.

**Delivered slice 2:** Live `target.base_url` discovery now crawls bounded
same-origin HTTP links and `/sitemap.xml` entries, then feeds those candidates
into `allie map`. This is static discovery only: HTTPS/TLS, credentialed crawl,
JavaScript navigation discovery, live agentic review, changed-surface inference,
and real authenticated-app dogfood remain in this epic.

**Delivered slice 3:** `flow.states[].steps` now lets a manifest perform
non-secret state setup actions (`fill`, `type`, `click`, `wait_for`) before
browser evidence capture. Auth/login secrets still belong in `auth.steps` via
env-var names; state steps are for deterministic UI setup such as opening menus
or waiting for panels.

**Why:** real-app-proving lane (auth preflight-only `lib.rs:2862`; worker goto-only `run.mjs:128`; no credential serialization `lib.rs:3029`) + autonomy-depth lane (discovery fixture-only `lib.rs:1915`; workbench review offline `workbench.rs:390`). Operator priority #1. Land the worker-adapter extraction (epic 019, child 1) first to give child 1 here a clean seam.
