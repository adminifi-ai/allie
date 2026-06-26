#!/bin/sh
# Authenticated-audit smoke (ticket 023, epic 015 slice 1).
#
# Proves three things, with no real secret on disk:
#   POSITIVE  — log into the fixture, reach the gated /dashboard.html, record the
#               per-state auth assertion as satisfied (final URL is the gated
#               route AND #dashboard present), packet passes.
#   SECRETS   — the credential VALUE never appears in worker-request.json, the
#               evidence packet, or any artifact (screenshots / DOM / trace).
#   NEGATIVE  — without a real session the gated route bounces to the HTTP-200
#               login wall; the worker records an `auth-lost` state_error and the
#               run BLOCKS (non-zero exit / fail status). A login wall must NOT
#               be audited as if it were the app.
set -eu

POS_DIR=.allie/runs/auth-smoke
NEG_DIR=.allie/runs/auth-smoke-neg
SECRET=allie-fixture-pass

rm -rf "$POS_DIR" "$NEG_DIR"

# --- POSITIVE -------------------------------------------------------------
ALLIE_AUTH_FIXTURE_USER=allie@example.com \
ALLIE_AUTH_FIXTURE_PASSWORD="$SECRET" \
  cargo run --locked -- run \
    --manifest examples/auth-fixture-flow.yml \
    --out "$POS_DIR"

POS_DIR="$POS_DIR" node -e "
const fs = require('fs');
const p = JSON.parse(fs.readFileSync(process.env.POS_DIR + '/evidence.json', 'utf8'));
if (!['pass', 'approved', 'passed'].includes(p.summary.status)) {
  throw new Error('positive packet status not approved/passed: ' + p.summary.status);
}
const dashboard = p.coverage.state_metadata.find((s) => s.id === 'dashboard');
if (!dashboard) throw new Error('positive run missing dashboard state');
if (!dashboard.url.endsWith('dashboard.html')) {
  throw new Error('dashboard state did not land on dashboard.html: ' + dashboard.url);
}
if (dashboard.state_errors.length !== 0) {
  throw new Error('dashboard state_errors not empty: ' + JSON.stringify(dashboard.state_errors));
}
"

# --- SECRETS: the credential value must appear in NO file under POS_DIR ----
# Scope: the positive run, where login succeeds and the audited DOM is the
# dashboard. (We do not grep the negative run dir: it bounces to the fixture's
# own login.html, whose static source embeds the fixture's accept-password as a
# client-side comparison constant — that is the fixture defining its own test
# credential, not Allie writing an injected secret to disk.)
if grep -rIn "$SECRET" "$POS_DIR" >/dev/null 2>&1; then
  echo "FAIL: credential value leaked into $POS_DIR:"
  grep -rIn "$SECRET" "$POS_DIR" || true
  exit 1
fi
# The env NAME must still be present in the request (so the worker can read it).
grep -q "ALLIE_AUTH_FIXTURE_PASSWORD" "$POS_DIR/worker-request.json"

# --- NEGATIVE CONTROL -----------------------------------------------------
# Use the no-session manifest so the gated route bounces to the HTTP-200 login
# wall and the per-state auth assertion fails (auth-lost). `run` exits 1 on a
# blocking finding, so guard the non-zero exit.
set +e
ALLIE_AUTH_FIXTURE_USER=allie@example.com \
ALLIE_AUTH_FIXTURE_PASSWORD=wrong-pass \
  cargo run --locked -- run \
    --manifest examples/auth-fixture-flow-negative.yml \
    --out "$NEG_DIR"
neg_status=$?
set -e
if [ "$neg_status" -eq 0 ]; then
  echo "FAIL: negative control passed but must block (HTTP-200 login wall)."
  exit 1
fi

NEG_DIR="$NEG_DIR" node -e "
const fs = require('fs');
const p = JSON.parse(fs.readFileSync(process.env.NEG_DIR + '/evidence.json', 'utf8'));
if (['pass', 'approved', 'passed'].includes(p.summary.status)) {
  throw new Error('negative control must not pass: ' + p.summary.status);
}
if (!p.summary.failure_class) throw new Error('negative control failure_class not set');
const dashboard = p.coverage.state_metadata.find((s) => s.id === 'dashboard');
if (!dashboard) throw new Error('negative run missing dashboard state');
if (!dashboard.state_errors.some((e) => e.includes('auth'))) {
  throw new Error('dashboard state missing an auth state_error: ' + JSON.stringify(dashboard.state_errors));
}
"

test -f "$POS_DIR/evidence.json"
test -f "$NEG_DIR/evidence.json"
echo "auth smoke passed (positive reached gated route, secrets clean, negative blocked)"
