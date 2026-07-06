#!/bin/sh
# Prove Allie can run from a bundled binary layout without source-checkout env
# wiring: bundle/bin/allie resolves bundle/workers/browser/run.mjs.
set -eu

# Post-AL-082, `allie init` auto-enables model review when a provider API key
# resolves in the environment. This smoke is meant to be offline-deterministic,
# so isolate it from any ambient keys rather than let the scaffolded manifest
# vary with whatever a developer's shell happens to export.
unset OPENROUTER_API_KEY OPENAI_API_KEY

ALLIE_REPO="$(pwd)"
OUT="$ALLIE_REPO/.allie/distribution-smoke"
BUNDLE="$OUT/bundle/allie"
WORK="$OUT/consumer"
BIN="$BUNDLE/bin/allie"

rm -rf "$OUT"
mkdir -p "$BUNDLE/bin" "$BUNDLE/workers" "$BUNDLE/fixtures" "$WORK/.allie"

cargo build --locked
cp "$ALLIE_REPO/target/debug/allie" "$BIN"
cp -R "$ALLIE_REPO/workers/browser" "$BUNDLE/workers/browser"
cp -R "$ALLIE_REPO/fixtures/login" "$BUNDLE/fixtures/login"
cp "$ALLIE_REPO/package.json" "$BUNDLE/package.json"
cp "$ALLIE_REPO/package-lock.json" "$BUNDLE/package-lock.json"
cp -R "$ALLIE_REPO/node_modules" "$BUNDLE/node_modules"

"$BIN" init \
  --manifest "$WORK/.allie/manifest.yml" \
  --app-name "Bundled Allie Smoke" \
  --fixture-dir "$BUNDLE/fixtures/login" \
  --force

git -C "$WORK" init -q
git -C "$WORK" config user.email "allie-smoke@example.invalid"
git -C "$WORK" config user.name "Allie Smoke"
git -C "$WORK" add .allie/manifest.yml
git -C "$WORK" commit -q -m "bundled allie fixture manifest"

cd "$WORK"
unset ALLIE_BROWSER_WORKER
"$BIN" doctor --manifest .allie/manifest.yml --out .allie/doctor
test -f .allie/doctor/doctor.json
"$BIN" run --manifest .allie/manifest.yml --out .allie/run/latest || true

EVID=".allie/run/latest/evidence.json"
test -f "$EVID"
node - "$EVID" <<'NODE'
import fs from 'node:fs';
const evid = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const captured = evid.summary?.states_captured ?? 0;
const infra = evid.summary?.infrastructure_failures ?? 0;
if (captured < 1) {
  throw new Error(`bundled distribution captured no states (states_captured=${captured})`);
}
if (infra > 0) {
  throw new Error(`bundled distribution had infrastructure_failures=${infra}`);
}
console.log(`distribution smoke ok: states_captured=${captured}, infrastructure_failures=${infra}`);
NODE

echo "distribution smoke passed: $WORK/.allie/run/latest"
