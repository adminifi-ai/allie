# Olympus Dashboard Dogfood

Date: 2026-07-01
Ticket: 025

## Target

This is the first substantive real-app slice for the dogfood ladder. Olympus was
selected because it is a clean Adminifi operations/control-plane app with
server-rendered Hono dashboard surfaces and cookie auth.

- Target checkout used for this run: `/Users/phaedrus/Development/adminifi/olympus`
- Target commit: `a743f88f947cea8a925e8b3e12fc767ccca4641a`
- Target status before run: `## main...origin/main`
- Target status after run: `## main...origin/main`
- Node runtime: `mise exec node@22.15.0`
- Local base URL: `http://127.0.0.1:56452` (`PORT=56452`)
- Auth method: dashboard password via `DASHBOARD_SECRET` env name only
- Local DB: disposable SQLite file under Allie's ignored `.allie/dogfood/olympus/`

Full Olympus boot was attempted with inert local env and intentionally stopped
as unsafe for this audit path: the real orchestrator verifies Sprite lanes
before serving and failed on fake Sprites auth. The dogfood therefore used
`olympus-dashboard-harness.mjs`, which imports Olympus' real dashboard router
and real SQLite schema but skips lane boot/dispatch. This avoids production
tokens and keeps the target repo untouched.

## Commands

Replay variables:

```sh
ALLIE_ROOT=/path/to/allie
OLYMPUS_ROOT=/path/to/olympus
PORT=56452
DB_PATH="${ALLIE_ROOT}/.allie/dogfood/olympus/olympus-harness.db"
DASHBOARD_SECRET=<local-dashboard-password>
```

The committed manifests intentionally pin `http://127.0.0.1:56452`; keep
`PORT=56452` for this slice unless the manifests are updated in the same change.

Start the local dashboard harness from the Olympus orchestrator directory:

```sh
cd "${OLYMPUS_ROOT}/orchestrator"
PORT="${PORT}" \
DB_PATH="${DB_PATH}" \
DASHBOARD_SECRET="${DASHBOARD_SECRET}" \
OLYMPUS_ORCHESTRATOR_ROOT="${OLYMPUS_ROOT}/orchestrator" \
mise exec node@22.15.0 -- npm exec -- tsx \
  "${ALLIE_ROOT}/docs/dogfood/olympus/olympus-dashboard-harness.mjs"
```

Capture the unauthenticated login page:

```sh
cargo run --locked -- run \
  --manifest docs/dogfood/olympus/025-olympus-login-smoke.yml \
  --out .allie/dogfood/olympus/login-smoke
```

Run the authenticated dashboard verify:

```sh
DASHBOARD_SECRET="${DASHBOARD_SECRET}" \
  cargo run --locked -- verify \
    --manifest docs/dogfood/olympus/025-olympus-dashboard-auth-flow.yml \
    --out .allie/dogfood/olympus/dashboard-auth-verify \
    --project-root "${OLYMPUS_ROOT}"
```

Run the negative auth control:

```sh
DASHBOARD_SECRET=<wrong-local-password> \
  cargo run --locked -- run \
    --manifest docs/dogfood/olympus/025-olympus-dashboard-auth-flow.yml \
    --out .allie/dogfood/olympus/dashboard-auth-negative
```

## Evidence

Generated receipt paths:

- Login evidence packet: `.allie/dogfood/olympus/login-smoke/evidence.json`
- Login report: `.allie/dogfood/olympus/login-smoke/report.html`
- Authenticated summary JSON: `.allie/dogfood/olympus/dashboard-auth-verify/reporters/allie-report.json`
- Authenticated verify HTML: `.allie/dogfood/olympus/dashboard-auth-verify/reporters/allie-report.html`
- Product map: `.allie/dogfood/olympus/dashboard-auth-verify/map/product-map.json`
- Surface map: `.allie/dogfood/olympus/dashboard-auth-verify/map/surface-map.html`
- Evidence packet: `.allie/dogfood/olympus/dashboard-auth-verify/run/evidence.json`
- WCAG report JSON: `.allie/dogfood/olympus/dashboard-auth-verify/report/compliance-report.json`
- WCAG report HTML: `.allie/dogfood/olympus/dashboard-auth-verify/report/compliance-report.html`
- Release summary: `.allie/dogfood/olympus/dashboard-auth-verify/release/release-summary.json`
- JUnit: `.allie/dogfood/olympus/dashboard-auth-verify/reporters/junit.xml`
- SARIF: `.allie/dogfood/olympus/dashboard-auth-verify/reporters/allie.sarif`
- Negative auth evidence: `.allie/dogfood/olympus/dashboard-auth-negative/evidence.json`

Login smoke facts:

- Evidence status: `fail`
- Exit code: `1`
- States captured: `1`
- Infrastructure failures: `0`
- Deterministic failures: `2`
- Finding class: contrast minimum on the unauthenticated login page

Authenticated verify facts:

- Verify status: `blocked`
- Evidence status: `fail`
- Evidence exit code: `1`
- Failure class: `blocking-finding`
- States captured: `6`
- Infrastructure failures: `0`
- Scripted failures: `0`
- Deterministic failures: `23`
- State errors: `0`
- Console errors: `0`
- Network errors: `0`
- Release status: `blocked`
- WCAG report: `fail` (`55` obligations: `0` pass, `3` fail, `44`
  needs_review, `8` not_tested)

Routes captured:

- `/dashboard`
- `/dashboard/agents`
- `/dashboard/billing`
- `/dashboard/dead-letters`
- `/dashboard/observability`
- `/dashboard/recorder`

Deterministic finding classes:

- `wcag22-aa:1.4.3-contrast-minimum`: `12` findings
- `wcag22-aa:1.3.1-info-and-relationships`: `8` findings
- `wcag22-aa:2.1.1-keyboard`: `3` findings

Negative auth control:

- Evidence status: `error`
- Exit code: `2`
- Infrastructure failures: `1`
- States captured: `0`
- Worker error: `auth-failed at step 2 (wait_for)`

## Interpretation

Allie successfully produced a replayable evidence packet, product map, surface
map, WCAG report, release projection, JUnit, and SARIF for a real Hono
operations dashboard. The authenticated run reached all six dashboard pages at
HTTP 200 with no auth loss, no state errors, no console errors, and no network
errors. The blocked release decision came from deterministic target findings,
not from an Allie infrastructure failure.

The negative auth control proved the no-silent-gaps behavior: with the wrong
dashboard password, Allie captured zero dashboard states and failed during auth
setup instead of auditing the login wall as if it were the app.

## Residual Risk

- The harness uses Olympus' real dashboard router and schema, but not the full
  orchestrator boot path, because full boot verifies Sprite lanes before serving.
- The harness imports Olympus internal dashboard modules
  (`src/db.ts`, `src/dashboard/route.tsx`) because Olympus has no stable
  dashboard-only export. This receipt pins target commit
  `a743f88f947cea8a925e8b3e12fc767ccca4641a`; if those paths move, update the
  dogfood shim or replace it with a target-owned dashboard adapter.
- The disposable DB contains no real job/run rows, so job detail, run detail,
  and report-artifact pages remain unexercised.
- This is evidence visibility only, not a legal compliance claim.
- All generated `.allie/dogfood/olympus/` artifacts are local ignored state and
  should be regenerated when reviewing the receipt.
