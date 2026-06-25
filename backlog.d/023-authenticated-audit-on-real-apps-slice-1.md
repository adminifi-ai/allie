# Authenticated audit on a real app (015 slice 1)

Priority: P0 · Status: ready · Estimate: L

> Shaped slice 1 of epic [015](015-prove-allie-on-real-authenticated-apps.md).
> This is the `/deliver`-ready context packet; later children of 015 (live
> crawl, changed-surface-from-git) stay deferred.

## Goal
Allie logs into a real authenticated web app, holds the session, and produces a
trustworthy WCAG evidence packet for hand-listed authenticated routes — with
credential values never written to disk or artifacts.

## Non-Goals
- Live / autonomous crawl and discovery (015 child 3) — routes are hand-listed in the manifest for this slice.
- Changed-surface inference from git diff (015 child 6).
- Automating a full SSO / OAuth redirect flow — covered instead by a `storageState` escape hatch.
- The `src/lib.rs` worker-adapter extraction (epic 019) — sequence it separately; do not bundle.
- Multi-surface agentic-review scale (epic 016).

## Constraints (invariants that must survive)
- **Secrets never persist.** No credential value in `worker-request.json`, the evidence packet, error messages, or any artifact. Preserve and extend the guarantee in the existing test (`src/lib.rs:4812`).
- **No silent gaps.** If `auth` is declared but a session is not established, the run BLOCKS (non-zero exit / blocked status) — it must never emit a passing packet that audits the login wall as if it were the app.
- **Determinism.** The new fixture-backed proof path is byte-stable (no wall-clock / ordering nondeterminism).
- **Boundary holds.** Playwright specifics stay in the `node` worker; Rust only extends the request schema. No new provider/browser leakage into `lib.rs`.
- Existing smokes stay green (worker / evidence / consumer / autonomous / size).

## Repo Anchors
- `src/lib.rs:2763-3082` — `FlowManifest`, `CredentialConfig`, `ManifestState`, `WorkerRequest`/`WorkerTarget` (the manifest + IPC contract to extend with `auth`).
- `src/lib.rs:2862-2893` — `preflight_failures` (stop the "set but unused" theater; require + actually use the auth env vars).
- `src/lib.rs:900-940` — `run` subcommand: `invoke_worker` (910) + `write_packet_and_report` (933).
- `workers/browser/run.mjs:51-110` — `runWorker` (context creation; perform login here, once per context).
- `workers/browser/run.mjs:112-129, 298-320` — `inspectState` goto + `resolveTarget` (the `base_url` path the dogfood uses).
- `examples/login-flow.yml` — manifest exemplar to mirror for the new auth fixture flow.
- `src/lib.rs:4812` — existing test: missing creds writes an error packet WITHOUT secret values (the contract to preserve/extend to the auth path).
- `package.json:7-17` + `scripts/coverage-matrix-smoke.sh` — smoke-script pattern for the new `auth:smoke`.

## Alternatives
1. **Login recipe in manifest; secret values via the worker's inherited env (CHOSEN).** Recipe (selectors, paths, env-var *names*) rides in `WorkerRequest`; the `node` child reads secret *values* from `process.env`. Secrets never on disk. Fails only on exotic SSO → mitigated by the storageState hatch.
2. **Pass resolved secret values inside the `WorkerRequest` JSON.** Simpler wiring but writes secrets to disk; breaks the `lib.rs:4812` contract. Rejected.
3. **storageState-only** (operator logs in manually, hands Allie a cookie file). Works for any auth incl. SSO, but not autonomous and adds manual work each run. **Kept as the escape hatch**, not the primary.
4. **Boring path: audit only public pages of a real app.** Rejected — never exercises authenticated surfaces, which is the entire point of 015.
5. **Do the 019 worker-adapter refactor first.** Cleaner seam but bundles an XL refactor into the keystone (delete-first/ponytail says no). The new auth code stays cohesive and becomes part of what 019 later extracts.

## Design
Add an optional `auth` block to `FlowManifest` (mirrors `CredentialConfig`'s env discipline):

```yaml
auth:
  start_path: /login
  steps:
    - fill:     { selector: "#email",    value_env: ALLIE_VANITY_USER }
    - fill:     { selector: "#password", value_env: ALLIE_VANITY_PASSWORD }
    - click:    { selector: "button[type=submit]" }
    - wait_for: { selector: "#dashboard" }       # success signal: selector OR url_contains
  storage_state_env: ALLIE_VANITY_STORAGE_STATE  # optional SSO hatch: env names a storageState path
```

- **Step vocabulary stays minimal:** `fill {selector, value_env}`, `click {selector}`, `wait_for {selector | url_contains}`. This is a *deterministic worker-facing* contract — the worker branches on step kind, so a typed step shape earns its place. (Contrast: the model-facing audit report stays unstructured prose. Structure only where code branches.)
- **Rust:** `WorkerRequest::from_manifest` copies `manifest.auth` → a `WorkerAuth` carrying selectors/paths/env *names* (never values). Extend `preflight_failures` to require each referenced `value_env` (or the `storage_state_env` path) when `auth` is present. Treat a worker auth error as a blocking `RunFailure`.
- **Worker (`run.mjs`):** in `runWorker`, after `browser.newContext(...)`: if `auth.storage_state_env` resolves to a readable file, create the context with `storageState`; else open a page, `goto(start_path)`, execute steps (reading `process.env[value_env]`), assert the success signal, close the login page (session cookies remain in the context), then inspect states as today. On failure return `status:"error"` / a dedicated `auth-failed` with the failing step but NO secret values.
- **Entrypoint:** flows through the existing `run` subcommand (`lib.rs:900-940`) → `invoke_worker` → `write_packet_and_report`. `allie verify` inherits it automatically (it composes `run`).

## Oracle
- [ ] `fixtures/auth/` static fixture (login form sets a cookie via JS; `/dashboard` JS-redirects to `/login` without it) and `examples/auth-fixture-flow.yml` exist.
- [ ] `scripts/auth-smoke.sh` + `npm run auth:smoke`: logs into the fixture, reaches gated `/dashboard` authenticated, and the evidence packet captures the dashboard state (not the login redirect).
- [ ] Negative control (no/bad creds on the gated route) → run BLOCKS (non-zero / blocked status), not a passing audit of the login wall.
- [ ] Secret-grep clean: the credential values appear in zero of `worker-request.json`, the evidence packet, and artifacts.
- [ ] storageState hatch: a manifest with `auth.storage_state_env` loads the session and audits a gated route.
- [ ] `cargo fmt --check`, `cargo test --locked`, `cargo clippy --locked -- -D warnings` pass; `npm run worker:smoke evidence:smoke consumer:smoke autonomous:smoke size:smoke` stay green.
- [ ] Live dogfood: `allie run` (or `verify`) against `phrazzld/vanity` (base_url + form login or storageState) produces an evidence packet for ≥1 authenticated route; receipt committed under `docs/dogfood/vanity/`. If vanity has no gated surface, escalate to `misty-step/misty-step`.

## Verification System
- **Claim:** Allie logs into a real authenticated web app and audits authenticated surfaces, secrets never persisted.
- **Falsifier:** (a) packet audits only the login page / unauth routes; (b) auth declared but no session established yet the run "passed"; (c) any credential value appears in request/packet/artifacts.
- **Driver:** `npm run auth:smoke` (reproducible, no secrets) + `allie run --manifest <vanity>.yml` (live dogfood).
- **Grader:** smoke asserts gated route reached + negative control blocks + secret-grep clean; dogfood receipt manually inspected for authenticated content.
- **Evidence packet:** `.allie/runs/auth-smoke/` (CI) + `docs/dogfood/vanity/` (live receipt).
- **Cadence:** `auth:smoke` per-change in CI (also feeds epic 021); dogfood receipt refreshed at each 015 milestone.
- **Gaps/waiver:** SSO/OAuth automation deferred to the storageState hatch; live crawl deferred to a follow-on slice.

## Premise Source
`sha256:93ae1849bbd13024b7244e7df6ec48943549f0a897f3804c5304745423bea1f5 backlog.d/015-prove-allie-on-real-authenticated-apps.md` (epic 015, committed `da4448c`), refined by operator interview 2026-06-25 (slice scope = auth + declared routes; target = phrazzld/vanity with the misty-step/adminifi ladder behind it; auth = form login + storageState hatch).

## HTML Plan
`/private/tmp/claude-501/-Users-phaedrus-Development-allie/e653f701-e37e-40bd-b58d-0d8842040f22/scratchpad/015-slice1-plan.html` (authored and opened for review).

## Risks + Rollout
- **Real-app login flaky (selectors drift):** storageState hatch + clear `auth-failed` error naming the failing step (no secret values).
- **Secrets leak via authed-page screenshot/DOM:** existing redaction policy; dogfood with a throwaway staging account + redacted artifacts.
- **vanity has no gated surface:** escalate up the ladder (misty-step → habitat/chrondle/linejam).
- **Rollout:** additive (new optional manifest block, new fixture, new smoke); existing fixture flows untouched. Revert = remove auth handling.
- **Stop conditions (come back, don't improvise):** target needs real SSO/OAuth and no storageState can be captured; the static fixture can't model the target's auth shape; redaction can't keep authed-page artifacts safe; auth would force the 019 worker-adapter refactor to land first.
