# Authenticated Audit Dogfood

Date: 2026-06-29
Ticket: 023, epic 015 slice 1

## Target Ladder

The ticket named `phrazzld/vanity` first, then `misty-step`, then the
adminifi/Linejam/Chrondle ladder if Vanity had no gated surface.

- Vanity remained a static/public local checkout for Allie purposes. Existing
  receipt: `docs/dogfood/014-vanity-consumer-contract.md`; no authenticated
  route or login wall was available to exercise.
- Misty Step was not selected for the run because the local app did not expose a
  deterministic form-login or reusable storageState path in the inspected app
  surface.
- Habitat exposes middleware-gated routes, but the browser login is Supabase
  magic-link based. That is a valid storageState target, not a deterministic
  form-login target without operator mailbox interaction.
- Linejam exposes a Clerk-authenticated `/me/profile` route and already has
  local test Clerk keys plus an existing smoke user. This was the first target
  on the ladder with a safe, scriptable authenticated browser session.

## Commands

Clerk key and smoke-user probe, without printing secret values:

```sh
node --input-type=module - <<'NODE'
// loaded Linejam env files, then printed only booleans/key family/user-exists
NODE
```

Result:

```json
{
  "hasSecretKey": true,
  "hasPublishableKey": true,
  "secretKeyFamily": "test",
  "publishableKeyFamily": "test",
  "smokeUserExists": true
}
```

Allie fixture proof used for the CI-falsifiable path:

```sh
npm run auth:smoke
```

Linejam authenticated storageState dogfood:

```sh
PORT=3333 pnpm dev:next
node --input-type=module - <<'NODE'
// loaded Linejam env files, signed in the existing Clerk smoke user, asserted
// /me/profile rendered "Identity", and wrote a disposable Playwright
// storageState JSON under Allie's ignored .allie run directory.
NODE
ALLIE_LINEJAM_STORAGE_STATE=.allie/runs/linejam-auth-dogfood/linejam-storage-state.json \
  cargo run --locked -- run \
    --manifest .allie/runs/linejam-auth-dogfood/manifest.yml \
    --out .allie/runs/linejam-auth-dogfood/run
```

Generated evidence:

- Form-login packet: `.allie/runs/auth-smoke/evidence.json`
- StorageState packet: `.allie/runs/auth-smoke-storage-state/evidence.json`
- Negative-control packet: `.allie/runs/auth-smoke-neg/evidence.json`
- Linejam packet: `.allie/runs/linejam-auth-dogfood/run/evidence.json`
- Linejam report: `.allie/runs/linejam-auth-dogfood/run/report.html`

## Result

`npm run auth:smoke` now proves all three authenticated-audit paths:

- form login reaches `/dashboard.html` and satisfies `#dashboard`;
- storageState reaches `/dashboard.html` and satisfies `#dashboard` without
  login steps;
- no-session negative control records `auth-lost` and blocks instead of
  auditing the HTTP-200 login wall.

The live-app ladder selected Linejam for the authenticated real-app dogfood
because it had test Clerk auth available without storing passwords in Allie
manifests. The run used a captured storageState session generated outside
Allie's artifacts and then passed only the storageState path through
`ALLIE_LINEJAM_STORAGE_STATE`.

Linejam dogfood result: `allie run` exited `1` with `status: fail` because the
authenticated profile route produced one deterministic reflow finding. The auth
contract held: the captured state was `/me/profile`, HTTP status was `200`,
`state_errors` was empty, the final URL contained `/me/profile`, and the marker
selector was present. The packet reported zero axe violations, zero console
errors, and zero network errors for the state.

Receipt note: the storageState file is local, ignored, and disposable. The
Allie manifest and worker request carry only env var names, not session or
credential values.

## Residual Risk

The committed CI proof uses the deterministic fixture. Clerk/SSO-style real app
auth is intentionally covered by the storageState hatch; refreshing that live
receipt requires the target's test auth keys and smoke account to remain valid.
During the capture phase, Linejam's dev server logged Clerk development-key and
refresh-loop warnings; the subsequent Allie run still reached the authenticated
route with no auth state errors, so this is target-environment risk rather than
an Allie worker failure.
