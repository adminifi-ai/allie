# Linejam Authenticated Verify Dogfood

Date: 2026-06-30
Ticket: 015

## Target

Linejam was selected as the real authenticated app because it exposes a
Clerk-protected `/me/profile` route and already has local test Clerk keys plus a
smoke user path. The target repo was not edited for this run.

- Target checkout: `/Users/phaedrus/Development/linejam`
- Target route: `http://localhost:3333/me/profile`
- Auth method: Playwright storageState generated with Linejam's Clerk test
  helper and passed to Allie through `ALLIE_LINEJAM_STORAGE_STATE`
- Storage note: the storageState file is ignored local state and is not
  committed

## Commands

Start Linejam:

```sh
cd /Users/phaedrus/Development/linejam
PORT=3333 pnpm dev:next
```

Create a fresh local storageState using Linejam's Clerk test keys before handing
the session to Allie. The setup helper loaded Linejam `.env` files without
printing values, refused non-test Clerk keys, ensured the smoke user existed,
signed in with `@clerk/testing/playwright`, asserted `/me/profile` rendered the
`Identity` heading, and wrote the storageState under Allie's ignored `.allie`
directory.

Run the end-to-end consumer pipeline:

```sh
ALLIE_LINEJAM_STORAGE_STATE=.allie/verify/linejam-auth-dogfood/linejam-storage-state.json \
  cargo run --locked -- verify \
    --manifest .allie/runs/linejam-auth-dogfood/manifest.yml \
    --out .allie/verify/linejam-auth-dogfood-e2e \
    --project-root /Users/phaedrus/Development/linejam
```

Result: `allie verify` exited `1` with status `blocked`. That is the expected
product result because the authenticated page produced deterministic evidence,
not an infrastructure/auth failure.

## Evidence

Generated receipt paths:

- Summary JSON: `.allie/verify/linejam-auth-dogfood-e2e/reporters/allie-report.json`
- Verify HTML: `.allie/verify/linejam-auth-dogfood-e2e/reporters/allie-report.html`
- Product map: `.allie/verify/linejam-auth-dogfood-e2e/map/product-map.json`
- Surface map: `.allie/verify/linejam-auth-dogfood-e2e/map/surface-map.html`
- Evidence packet: `.allie/verify/linejam-auth-dogfood-e2e/run/evidence.json`
- WCAG report JSON: `.allie/verify/linejam-auth-dogfood-e2e/report/compliance-report.json`
- WCAG report HTML: `.allie/verify/linejam-auth-dogfood-e2e/report/compliance-report.html`
- Release summary: `.allie/verify/linejam-auth-dogfood-e2e/release/release-summary.json`
- JUnit: `.allie/verify/linejam-auth-dogfood-e2e/reporters/junit.xml`
- SARIF: `.allie/verify/linejam-auth-dogfood-e2e/reporters/allie.sarif`

Observed packet facts:

- Verify status: `blocked`
- Evidence status: `fail`
- Evidence exit code: `1`
- Failure class: `blocking-finding`
- States captured: `1`
- Infrastructure failures: `0`
- Scripted failures: `0`
- Deterministic failures: `1`
- Route captured: `/me/profile`
- Final URL: `http://localhost:3333/me/profile`
- HTTP status: `200`
- State errors: `0`
- Console errors: `0`
- Network errors: `0`
- WCAG report: `fail` (`55` obligations: `0` pass, `2` fail, `53`
  needs_review, `0` not_tested)
- Reflow checked: `true`
- Reflow overflow observed: `true`
- Release status: `blocked`
- Missing required evidence: `0`

The release-blocking deterministic finding was
`wcag22-aa:1.4.3-contrast-minimum` on the authenticated profile route. The WCAG
report also recorded scripted reflow failure for `wcag22-aa:1.4.10-reflow`.

## Interpretation

This proves the remaining epic-015 oracle: Allie can run the composed
`allie verify` pipeline against a real authenticated local app, preserve the
packet/report/release receipts, and distinguish real audit findings from
auth/session infrastructure failure. The auth marker held because the captured
state reached `/me/profile` at HTTP 200 with no `auth-lost` state errors.

## Residual Risk

The receipt depends on Linejam's local test Clerk keys and smoke account staying
usable. Refreshing the receipt should regenerate storageState first; reusing an
old storageState can fail closed with a worker infrastructure error, as expected.
