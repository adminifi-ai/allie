#!/bin/sh
# Authenticated-audit smoke (epic 015 / ticket 023).
#
# Proves the same auth contract through both `allie run` and `allie verify`,
# with no real secret on disk:
#   POSITIVE  — log into the fixture, reach the gated /dashboard.html, record the
#               per-state auth assertion as satisfied (final URL is the gated
#               route AND #dashboard present), packet passes.
#   SECRETS   — the credential VALUE never appears in worker-request.json, the
#               evidence packet, or any artifact (screenshots / DOM / trace).
#   STORAGE   — load a captured Playwright storageState session through the
#               env-named hatch and reach the same gated route without login
#               steps.
#   NEGATIVE  — without a real session the gated route bounces to the HTTP-200
#               login wall; the worker records an `auth-lost` state_error and the
#               run BLOCKS (non-zero exit / fail status). A login wall must NOT
#               be audited as if it were the app.
set -eu

POS_DIR=.allie/runs/auth-smoke
STATE_DIR=.allie/runs/auth-smoke-storage-state
STATE_FILE=.allie/runs/auth-smoke-storage-state.json
NEG_DIR=.allie/runs/auth-smoke-neg
VERIFY_POS_DIR=.allie/verify/auth-smoke
VERIFY_STATE_DIR=.allie/verify/auth-smoke-storage-state
VERIFY_NEG_DIR=.allie/verify/auth-smoke-neg
SECRET=allie-fixture-pass

rm -rf "$POS_DIR" "$STATE_DIR" "$NEG_DIR" "$VERIFY_POS_DIR" "$VERIFY_STATE_DIR" "$VERIFY_NEG_DIR"
rm -f "$STATE_FILE"
trap 'rm -f "$STATE_FILE"' EXIT

assert_dashboard_packet() {
  PACKET_PATH="$1" LABEL="$2" node -e '
const fs = require("fs");
const p = JSON.parse(fs.readFileSync(process.env.PACKET_PATH, "utf8"));
const label = process.env.LABEL;
if (!["pass", "approved", "passed"].includes(p.summary.status)) {
  throw new Error(`${label} packet status not approved/passed: ${p.summary.status}`);
}
const dashboard = p.coverage.state_metadata.find((s) => s.id === "dashboard");
if (!dashboard) throw new Error(`${label} missing dashboard state`);
if (!dashboard.url.endsWith("dashboard.html")) {
  throw new Error(`${label} did not land on dashboard.html: ${dashboard.url}`);
}
if (dashboard.state_errors.length !== 0) {
  throw new Error(`${label} dashboard state_errors not empty: ${JSON.stringify(dashboard.state_errors)}`);
}
'
}

assert_auth_block_packet() {
  PACKET_PATH="$1" LABEL="$2" node -e '
const fs = require("fs");
const p = JSON.parse(fs.readFileSync(process.env.PACKET_PATH, "utf8"));
const label = process.env.LABEL;
if (["pass", "approved", "passed"].includes(p.summary.status)) {
  throw new Error(`${label} must not pass: ${p.summary.status}`);
}
if (!p.summary.failure_class) throw new Error(`${label} failure_class not set`);
const dashboard = p.coverage.state_metadata.find((s) => s.id === "dashboard");
if (!dashboard) throw new Error(`${label} missing dashboard state`);
if (!dashboard.state_errors.some((e) => e.includes("auth"))) {
  throw new Error(`${label} dashboard missing an auth state_error: ${JSON.stringify(dashboard.state_errors)}`);
}
'
}

assert_verify_status() {
  SUMMARY_PATH="$1" EXPECTED="$2" LABEL="$3" node -e '
const fs = require("fs");
const report = JSON.parse(fs.readFileSync(process.env.SUMMARY_PATH, "utf8"));
const status = report.status;
const label = process.env.LABEL;
if (process.env.EXPECTED === "not_blocked") {
  if (!["approved", "needs_review"].includes(status)) {
    throw new Error(`${label} verify status should be approved/needs_review, got ${status}`);
  }
} else if (status !== "blocked") {
  throw new Error(`${label} verify status should be blocked, got ${status}`);
}
'
}

# --- POSITIVE -------------------------------------------------------------
ALLIE_AUTH_FIXTURE_USER=allie@example.com \
ALLIE_AUTH_FIXTURE_PASSWORD="$SECRET" \
  cargo run --locked -- run \
    --manifest examples/auth-fixture-flow.yml \
    --out "$POS_DIR"

assert_dashboard_packet "$POS_DIR/evidence.json" "positive run"

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

# --- STORAGESTATE HATCH ---------------------------------------------------
# The fixture accepts a browser session cookie named allie_session=ok. Scope it
# to 127.0.0.1 so it applies to the worker's ephemeral fixture server port.
mkdir -p "$(dirname "$STATE_FILE")"
cat >"$STATE_FILE" <<'JSON'
{
  "cookies": [
    {
      "name": "allie_session",
      "value": "ok",
      "domain": "127.0.0.1",
      "path": "/",
      "expires": -1,
      "httpOnly": false,
      "secure": false,
      "sameSite": "Lax"
    }
  ],
  "origins": []
}
JSON

ALLIE_AUTH_FIXTURE_STORAGE_STATE="$STATE_FILE" \
  cargo run --locked -- run \
    --manifest examples/auth-fixture-storage-state-flow.yml \
    --out "$STATE_DIR"

assert_dashboard_packet "$STATE_DIR/evidence.json" "storageState run"
grep -q "ALLIE_AUTH_FIXTURE_STORAGE_STATE" "$STATE_DIR/worker-request.json"
if grep -rIn "$STATE_FILE" "$STATE_DIR" >/dev/null 2>&1; then
  echo "FAIL: storageState file path leaked into $STATE_DIR:"
  grep -rIn "$STATE_FILE" "$STATE_DIR" || true
  exit 1
fi

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

assert_auth_block_packet "$NEG_DIR/evidence.json" "negative run"

# --- VERIFY: POSITIVE -----------------------------------------------------
ALLIE_AUTH_FIXTURE_USER=allie@example.com \
ALLIE_AUTH_FIXTURE_PASSWORD="$SECRET" \
  cargo run --locked -- verify \
    --manifest examples/auth-fixture-flow.yml \
    --out "$VERIFY_POS_DIR" \
    --project-root fixtures/auth

assert_dashboard_packet "$VERIFY_POS_DIR/run/evidence.json" "positive verify"
assert_verify_status "$VERIFY_POS_DIR/reporters/allie-report.json" not_blocked "positive verify"
grep -q "ALLIE_AUTH_FIXTURE_PASSWORD" "$VERIFY_POS_DIR/run/worker-request.json"
if grep -rIn "$SECRET" "$VERIFY_POS_DIR" >/dev/null 2>&1; then
  echo "FAIL: credential value leaked into $VERIFY_POS_DIR:"
  grep -rIn "$SECRET" "$VERIFY_POS_DIR" || true
  exit 1
fi

# --- VERIFY: STORAGESTATE -------------------------------------------------
ALLIE_AUTH_FIXTURE_STORAGE_STATE="$STATE_FILE" \
  cargo run --locked -- verify \
    --manifest examples/auth-fixture-storage-state-flow.yml \
    --out "$VERIFY_STATE_DIR" \
    --project-root fixtures/auth

assert_dashboard_packet "$VERIFY_STATE_DIR/run/evidence.json" "storageState verify"
assert_verify_status "$VERIFY_STATE_DIR/reporters/allie-report.json" not_blocked "storageState verify"
grep -q "ALLIE_AUTH_FIXTURE_STORAGE_STATE" "$VERIFY_STATE_DIR/run/worker-request.json"
if grep -rIn "$STATE_FILE" "$VERIFY_STATE_DIR" >/dev/null 2>&1; then
  echo "FAIL: storageState file path leaked into $VERIFY_STATE_DIR:"
  grep -rIn "$STATE_FILE" "$VERIFY_STATE_DIR" || true
  exit 1
fi

# --- VERIFY: NEGATIVE CONTROL --------------------------------------------
set +e
ALLIE_AUTH_FIXTURE_USER=allie@example.com \
ALLIE_AUTH_FIXTURE_PASSWORD=wrong-pass \
  cargo run --locked -- verify \
    --manifest examples/auth-fixture-flow-negative.yml \
    --out "$VERIFY_NEG_DIR" \
    --project-root fixtures/auth
verify_neg_status=$?
set -e
if [ "$verify_neg_status" -eq 0 ]; then
  echo "FAIL: negative verify passed but must block (HTTP-200 login wall)."
  exit 1
fi

assert_auth_block_packet "$VERIFY_NEG_DIR/run/evidence.json" "negative verify"
assert_verify_status "$VERIFY_NEG_DIR/reporters/allie-report.json" blocked "negative verify"

test -f "$POS_DIR/evidence.json"
test -f "$STATE_DIR/evidence.json"
test -f "$NEG_DIR/evidence.json"
test -f "$VERIFY_POS_DIR/run/evidence.json"
test -f "$VERIFY_POS_DIR/reporters/allie-report.json"
test -f "$VERIFY_STATE_DIR/run/evidence.json"
test -f "$VERIFY_STATE_DIR/reporters/allie-report.json"
test -f "$VERIFY_NEG_DIR/run/evidence.json"
test -f "$VERIFY_NEG_DIR/reporters/allie-report.json"
echo "auth smoke passed (run+verify reached gated route, storageState reached gated route, secrets clean, negatives blocked)"
