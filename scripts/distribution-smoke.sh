#!/bin/sh
# Prove Allie can run from a bundled binary layout without source-checkout env
# wiring: bundle/bin/allie resolves both bundled worker assets.
set -eu

# Post-AL-082, `allie init` auto-enables model review when a provider API key
# resolves in the environment. This smoke is meant to be offline-deterministic,
# so isolate it from any ambient keys rather than let the scaffolded manifest
# vary with whatever a developer's shell happens to export.
unset OPENROUTER_API_KEY OPENAI_API_KEY

ALLIE_REPO="$(pwd)"
OUT="$ALLIE_REPO/.allie/distribution-smoke"
RELEASE="$OUT/release"
BUNDLE_ROOT="$OUT/bundle"
BUNDLE="$BUNDLE_ROOT/allie"
WORK="$OUT/consumer"
BIN="$BUNDLE/bin/allie"

rm -rf "$OUT"
mkdir -p "$BUNDLE_ROOT" "$WORK/.allie"

# Exercise the actual release packager, not a hand-built approximation. This
# proves the release-mode binary, both workers, Node dependencies, and the
# bundle-local Playwright browser cache travel in the published archive.
"$ALLIE_REPO/scripts/package-release.sh" "$RELEASE"
set -- "$RELEASE"/allie-*.tar.gz
if [ "$#" -ne 1 ] || [ ! -f "$1" ]; then
  echo "FAIL: expected exactly one release archive in $RELEASE" >&2
  exit 1
fi
ARCHIVE="$1"
for entry in \
  allie/bin/allie \
  allie/workers/browser/run.mjs \
  allie/workers/agentic/review.mjs; do
  count="$(tar -tzf "$ARCHIVE" | awk -v expected="$entry" '$0 == expected { count += 1 } END { print count + 0 }')"
  if [ "$count" -ne 1 ]; then
    echo "FAIL: release archive contains $count copies of $entry" >&2
    exit 1
  fi
done
tar -xzf "$ARCHIVE" -C "$BUNDLE_ROOT"
test -d "$BUNDLE/ms-playwright"

# The old defect embedded only CARGO_MANIFEST_DIR, then joined workers/... at
# runtime. Scan the exact checkout root across the extracted archive so that
# regression cannot hide behind the joined path or compression. The poison
# fixture proves this oracle catches that precise compile-time root leak.
if grep -R -a -F -- "$ALLIE_REPO" "$BUNDLE" >/dev/null; then
  echo "FAIL: release archive embeds the source checkout root" >&2
  exit 1
fi
POISON="$OUT/source-path-negative-control"
cp "$BIN" "$POISON"
printf '%s\n' "$ALLIE_REPO" >> "$POISON"
if ! grep -a -F -- "$ALLIE_REPO" "$POISON" >/dev/null; then
  echo "FAIL: source-root artifact negative control was not detected" >&2
  exit 1
fi

# Make the spawned bundled worker assert the exact environment supplied by the
# Rust adapter before delegating to the real worker. Its browser launch then
# uses the release archive's bundle-local Playwright cache.
mv "$BUNDLE/workers/agentic/review.mjs" "$BUNDLE/workers/agentic/review-real.mjs"
cat > "$BUNDLE/workers/agentic/review.mjs" <<'NODE'
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const here = path.dirname(fileURLToPath(import.meta.url));
const expected = path.resolve(here, '../../ms-playwright');
if (process.env.PLAYWRIGHT_BROWSERS_PATH !== expected) {
  throw new Error(`expected bundled PLAYWRIGHT_BROWSERS_PATH=${expected}, received ${process.env.PLAYWRIGHT_BROWSERS_PATH || '<unset>'}`);
}
await import('./review-real.mjs');
NODE

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
unset ALLIE_BROWSER_WORKER ALLIE_AGENTIC_WORKER
"$BIN" doctor --manifest .allie/manifest.yml --out .allie/doctor
test -f .allie/doctor/doctor.json
node - .allie/doctor/doctor.json "$BUNDLE" <<'NODE'
import fs from 'node:fs';
import path from 'node:path';
const receipt = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const bundle = fs.realpathSync(process.argv[3]);
for (const [name, relative] of [
  ['browser worker', 'workers/browser/run.mjs'],
  ['agentic worker', 'workers/agentic/review.mjs'],
]) {
  const check = receipt.checks.find((entry) => entry.name === name);
  const expected = fs.realpathSync(path.join(bundle, relative));
  if (check?.status !== 'ok' || !check.detail.includes(expected)) {
    throw new Error(`${name} was not independently resolved from the bundle: ${JSON.stringify(check)}`);
  }
}
NODE

# Use one model-enabled manifest for both the independent doctor failure and
# the offline agentic run. The doctor receives a resolvable dummy credential;
# the later worker run explicitly removes it to prove honest not-sent behavior.
cat > .allie/agentic-bundle.yml <<YAML
id: bundled-agentic
name: Bundled agentic worker
app_name: Bundled Allie Smoke
environment: local-fixture
target:
  kind: local_fixture
  fixture_dir: $BUNDLE/fixtures/login
policy:
  profile: wcag22-aa
  blocking_classes: [deterministic]
  worker_timeout_ms: 30000
model:
  enabled: true
  redaction: none
  provider_allowlist: [openrouter]
  zdr_required: false
  provider: openrouter
  model: offline-bundle-smoke
  api_key_env: ALLIE_DISTRIBUTION_AGENTIC_NO_KEY
  base_url: https://openrouter.ai/api/v1
  max_model_calls: 1
browser:
  viewport: {width: 1280, height: 900}
  color_scheme: light
  reduced_motion: reduce
  locale: en-US
  zoom: 1.0
flow:
  id: bundled-agentic-flow
  description: Bundled agentic worker smoke.
  states:
    - id: login-form
      path: /
      description: Login form.
      required: true
      axe: true
      screenshot: true
YAML

# Missing agentic assets must fail independently while the browser worker stays
# healthy and model credentials resolve. The sentinel must never enter any
# receipt or captured output.
DUMMY_KEY='allie-distribution-doctor-secret-sentinel'
mv "$BUNDLE/workers/agentic/review.mjs" "$BUNDLE/workers/agentic/review.mjs.off"
if env ALLIE_DISTRIBUTION_AGENTIC_NO_KEY="$DUMMY_KEY" "$BIN" doctor \
  --manifest .allie/agentic-bundle.yml \
  --out .allie/doctor-missing-agentic \
  >.allie/doctor-missing-agentic.stdout \
  2>.allie/doctor-missing-agentic.stderr; then
  echo "FAIL: doctor accepted a bundle with no agentic worker" >&2
  exit 1
fi
node - \
  .allie/doctor-missing-agentic/doctor.json \
  .allie/doctor-missing-agentic.stdout \
  .allie/doctor-missing-agentic.stderr \
  "$DUMMY_KEY" <<'NODE'
import fs from 'node:fs';
const receipt = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const status = Object.fromEntries(receipt.checks.map((check) => [check.name, check.status]));
if (status['browser worker'] !== 'ok' || status['agentic worker'] !== 'fail' || status.model !== 'ok') {
  throw new Error(`doctor did not isolate the missing agentic worker: ${JSON.stringify(status)}`);
}
const captured = process.argv.slice(2, 5).map((file) => fs.readFileSync(file, 'utf8')).join('\n');
if (captured.includes(process.argv[5])) throw new Error('doctor leaked the dummy credential');
NODE
mv "$BUNDLE/workers/agentic/review.mjs.off" "$BUNDLE/workers/agentic/review.mjs"

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

# Exercise agentic review through the bundled binary with no source path or
# override. With no API key the worker still launches Playwright, captures
# review media, and records a truthful not-sent receipt without network egress.
env -u ALLIE_DISTRIBUTION_AGENTIC_NO_KEY "$BIN" verify \
  --manifest .allie/agentic-bundle.yml \
  --out .allie/verify/agentic-bundle
node - .allie/verify/agentic-bundle/run/agentic-response.json <<'NODE'
import fs from 'node:fs';
const response = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
if (response.status !== 'degraded' || response.redaction_receipt?.status !== 'not_sent') {
  throw new Error(`bundled agentic worker did not run offline: ${JSON.stringify(response)}`);
}
NODE

# Exercise the public projection from the bundled binary using a minimal verify
# reporter fixture. Distribution layout must not silently omit new CLI paths.
mkdir -p .allie/verify/latest/reporters
cp .allie/run/latest/evidence.json .allie/verify/latest/run-evidence-source.json
node - .allie/verify/latest/reporters/allie-report.json <<'NODE'
import fs from 'node:fs';
fs.writeFileSync(process.argv[2], JSON.stringify({
  schema: 'allie.verify.v0',
  status: 'needs_review',
  exit_code: 0,
  generated_at: '2026-07-15T00:00:00Z',
  release_status: 'needs_review',
  run_status: 'pass',
  why: {
    blocking: {
      deterministic_failures: 0,
      scripted_failures: 0,
      infrastructure_failures: 0,
      missing_required_evidence: [],
    },
    compliance_summary: {pass: 1, fail: 0, needs_review: 1, not_tested: 0},
  },
}));
NODE
"$BIN" publication \
  --verify-root .allie/verify/latest \
  --out .allie/public/latest
test -f .allie/public/latest/allie-public-summary.json
test -f .allie/public/latest/publication-receipt.json
test ! -f .allie/public/latest/run-evidence-source.json

echo "distribution smoke passed: $WORK/.allie/run/latest"
