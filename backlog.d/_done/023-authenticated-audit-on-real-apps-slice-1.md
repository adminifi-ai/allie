# Authenticated audit on a real app (015 slice 1)

Priority: P0 · Status: done · Estimate: L

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
- **No silent gaps.** If `auth` is declared but a session is not established, the run BLOCKS (non-zero exit / blocked status) — it must never emit a passing packet that audits the login wall as if it were the app. **HTTP status is not sufficient:** an SPA login wall returns HTTP 200 then JS-redirects, which today passes every check in `inspectState` (`run.mjs:128-135`). Enforcement requires the post-navigation auth assertion in Design, not status alone.
- **Determinism.** The new fixture-backed proof path is byte-stable (no wall-clock / ordering nondeterminism); the fixture's login gate redirects **synchronously** on load so `page.url()` is settled after `networkidle`.
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
  authenticated_marker: { selector: "#dashboard" }  # REQUIRED present on every gated state; its
                                                    # absence (e.g. bounced to /login) => auth lost => block
  storage_state_env: ALLIE_VANITY_STORAGE_STATE  # optional SSO hatch: env names a storageState path
```

- **Step vocabulary stays minimal:** `fill {selector, value_env}`, `click {selector}`, `wait_for {selector | url_contains}`. This is a *deterministic worker-facing* contract — the worker branches on step kind, so a typed step shape earns its place. (Contrast: the model-facing audit report stays unstructured prose. Structure only where code branches.)
- **Rust:** `WorkerRequest::from_manifest` copies `manifest.auth` → a `WorkerAuth` carrying selectors/paths/env *names* (never values). Extend `preflight_failures` to require each referenced `value_env` (or the `storage_state_env` path) when `auth` is present. Treat a worker auth error as a blocking `RunFailure`.
- **Worker (`run.mjs`):** in `runWorker`, after `browser.newContext(...)`: if `auth.storage_state_env` resolves to a readable file, create the context with `storageState`; else open a page, `goto(start_path)`, execute steps (reading `process.env[value_env]`), assert the success signal, close the login page (the JS-set session cookie persists in the context). Login-step failure → `status:"error"` / `auth-failed` naming the failing step but NO secret values.
- **Per-state auth assertion (the no-silent-gaps mechanism — added after critique):** in `inspectState`, when `authenticated_marker` is set, after `goto` assert the marker selector is present AND the final `page.url()` did not bounce to `start_path`; on failure push an `auth-lost` entry to `state_errors`. `state_errors` already flips the run to a blocking exit class (`run.mjs:91-95` → `lib.rs:3446-3455`), so a login wall served at HTTP 200 now blocks instead of being audited as the app. Without this, HTTP-200 status checks pass and the invariant is unenforced.
- **Entrypoint:** flows through the existing `run` subcommand (`lib.rs:900-940`) → `invoke_worker` → `write_packet_and_report`. `allie verify` inherits it automatically (it composes `run`).

## Oracle
- [x] `fixtures/auth/` static fixture (login form sets a cookie via JS; `/dashboard` JS-redirects to `/login` without it) and `examples/auth-fixture-flow.yml` exist.
- [x] `scripts/auth-smoke.sh` + `npm run auth:smoke`: logs into the fixture, reaches gated `/dashboard`, and the worker records the per-state auth assertion as satisfied (final URL is the gated route AND `authenticated_marker` present) — not the login redirect.
- [x] Negative control (no/bad creds on the gated route): the auth assertion fails → an `auth-lost` `state_error` is recorded → the run BLOCKS (non-zero / blocked exit class). Assert the exit class actually flips; an HTTP-200 login wall must NOT pass.
- [x] Secret-grep clean: a NEW test that actually sets a secret env var, runs the auth flow, then greps the value across `worker-request.json`, the evidence packet, screenshots, DOM snapshots, AND the trace file (`run.mjs:194` serializes URL + console/network errors — a token in a query string would leak there) → zero hits. (`lib.rs:4812` only covers the env-name path; its secret is never set, so this is a new case, not a reuse.)
- [x] storageState hatch: a manifest with `auth.storage_state_env` loads the session and audits a gated route.
- [x] `cargo fmt --check`, `cargo test --locked`, `cargo clippy --locked -- -D warnings` pass; `npm run worker:smoke evidence:smoke consumer:smoke autonomous:smoke size:smoke` stay green.
- [x] Live dogfood (manual receipt — not CI-falsifiable): `allie run` (or `verify`) against `phrazzld/vanity` (base_url + form login or storageState) produces an evidence packet for ≥1 authenticated route; receipt committed under `docs/dogfood/vanity/`. If vanity has no gated surface, escalate to `misty-step/misty-step`.

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

## Review
Fresh-context adversarial critique (2026-06-25, repo-grounded, different context) — verdict **fix-then-ship**. Fixes folded in:
- **Added the no-silent-gaps mechanism.** "Auth failure blocks" had no implementation: `inspectState` only checked HTTP status, so an HTTP-200 SPA login wall would pass. Added the `authenticated_marker` per-state assertion → `auth-lost` `state_error` → blocking exit class.
- **Hardened the secret-grep oracle.** `lib.rs:4812` only proves the env-*name* path (its secret is never set); the new test must set a real secret and grep all artifacts incl. the trace file (URL/query-string leak vector).
- **Clarified the fixture.** Gating is client-side JS (the static server can't gate); the redirect must be synchronous so `page.url()` is settled after `networkidle`. Marked the vanity-escalation oracle as a manual receipt.
- **Confirmed sound (no change):** secrets-via-inherited-env (`Command::new("node")` has no env scrub; `WorkerRequest` has no value field); worker exceptions → exit 2; one shared context carries the session across states.

Second fresh-context review (on the built diff, 2026-06-25) — verdict **ship, no blockers**. Nits fixed: stale `run.mjs` marker comment; added `trace: true` to the fixture state so the secret-grep actually covers the trace artifact (the noted leak vector). **Deferred to milestone 2 (the live dogfood):** the per-state marker `waitForSelector` uses a fixed 5s timeout — fine for the fixture, but a genuinely slow real authed page could false-positive as `auth-lost`; revisit deriving it from `policy.worker_timeout_ms` if real-app drift appears.

## Delivery Receipt

Delivered on 2026-06-29 in branch `deliver/023-authenticated-audit-slice1`.

- Added `examples/auth-fixture-storage-state-flow.yml` and extended
  `scripts/auth-smoke.sh` so `npm run auth:smoke` proves form login,
  storageState session loading, secret-grep cleanliness, and negative-control
  blocking.
- Added plan artifact `docs/plans/023-authenticated-audit-slice1.html`.
- Added real-app dogfood receipt
  `docs/dogfood/vanity/023-authenticated-audit-slice1.md`. Vanity had no
  authenticated route, so the ladder escalated to Linejam; Allie audited
  Linejam's Clerk-authenticated `/me/profile` route using captured storageState.
  The Linejam packet captured the authenticated route with HTTP 200 and no auth
  `state_errors`; it exited nonzero for a deterministic reflow finding in the
  target app, not for auth loss.

Verification:

```sh
npm run auth:smoke
cargo clippy --locked -- -D warnings
npm run verify
```
