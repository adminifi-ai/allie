#!/bin/sh
# Regression guard for the host-agnostic consumer contract.
#
# Allie's browser worker resolves its request/response/artifact paths against
# its OWN repoRoot (the Allie checkout). When Allie runs from a consumer repo
# (CWD != Allie checkout) the Rust side must therefore hand the worker ABSOLUTE
# paths; relative paths resolve under the Allie tree instead of the consumer's
# output dir and the worker crashes on a missing request (states_captured 0,
# infrastructure_failures 1). Every other smoke runs from inside the Allie repo
# (CWD == repoRoot) and so never exercised this path.
#
# This smoke reproduces a consumer repo with a temp dir, runs `allie` from that
# dir against the bundled login fixture, and fails unless the worker captured at
# least one real state with no infrastructure failure.
set -eu

ALLIE_REPO="$(pwd)"
BIN="$ALLIE_REPO/target/debug/allie"

cargo build --locked

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/.allie"

# Absolute fixture dir so the only foreign-CWD variable under test is the
# Rust -> worker request/response/artifact handshake.
"$BIN" init \
  --manifest "$WORK/.allie/manifest.yml" \
  --app-name "Consumer CWD Smoke" \
  --fixture-dir "$ALLIE_REPO/fixtures/login" \
  --force

cd "$WORK"
# `allie run` exits non-zero on findings; assert on captured evidence, not exit.
ALLIE_BROWSER_WORKER="$ALLIE_REPO/workers/browser/run.mjs" \
  "$BIN" run --manifest .allie/manifest.yml --out .allie/run/latest || true

EVID=".allie/run/latest/evidence.json"
test -f "$EVID"

node - "$EVID" <<'NODE'
import fs from 'node:fs';
const evid = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const captured = evid.summary?.states_captured ?? 0;
const infra = evid.summary?.infrastructure_failures ?? 0;
if (captured < 1) {
  throw new Error(`consumer-cwd run captured no states (states_captured=${captured}); the worker handshake broke from a foreign CWD`);
}
if (infra > 0) {
  throw new Error(`consumer-cwd run had infrastructure_failures=${infra}; expected the worker to run cleanly from a foreign CWD`);
}
console.log(`consumer-cwd smoke ok: states_captured=${captured}, infrastructure_failures=${infra}`);
NODE

echo "consumer cwd smoke passed: $WORK/.allie/run/latest"
