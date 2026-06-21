#!/bin/sh
# Deterministic guard for the agentic review gateway.
#
# Runs the gateway against the bundled login fixture with NO model API key, so
# there is no network call: it must still launch the browser, capture the
# evidence (screenshots), and return a well-formed response that marks the
# criteria "unavailable" rather than crashing or fabricating a verdict. This
# locks the gateway's capture + graceful-degradation path; the live model path
# is exercised by real verify runs, not the offline gate.
set -eu

ALLIE_REPO="$(pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

cat > "$WORK/request.json" <<JSON
{
  "schema": "allie.agentic.request.v0",
  "target": { "fixture_dir": "$ALLIE_REPO/fixtures/login" },
  "browser": { "viewport": { "width": 1024, "height": 768 }, "color_scheme": "light", "reduced_motion": "reduce", "locale": "en-US" },
  "model": { "provider": "openrouter", "model": "offline", "api_key_env": "ALLIE_AGENTIC_SMOKE_NO_KEY", "base_url": "https://openrouter.ai/api/v1", "max_calls": 1 },
  "artifacts_dir": "$WORK/artifacts",
  "criteria": [
    { "obligation": "wcag22-aa:2.4.7-focus-visible", "num": "2.4.7", "handle": "Focus Visible", "level": "AA", "principle": "Operable" }
  ]
}
JSON

env -u ALLIE_AGENTIC_SMOKE_NO_KEY node workers/agentic/review.mjs \
  --request "$WORK/request.json" --response "$WORK/response.json"

node - "$WORK/response.json" <<'NODE'
import fs from 'node:fs';
const response = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
if (response.schema !== 'allie.agentic.response.v0') {
  throw new Error(`unexpected response schema ${response.schema}`);
}
if (!Array.isArray(response.assessments) || response.assessments.length < 1) {
  throw new Error('gateway returned no assessments');
}
const assessment = response.assessments[0];
if (assessment.assessment !== 'unavailable') {
  throw new Error(`expected "unavailable" without an API key, got "${assessment.assessment}"`);
}
if (!assessment.media.some((entry) => entry.kind === 'screenshot')) {
  throw new Error('gateway did not capture screenshot evidence');
}
console.log(`agentic smoke ok: captured ${assessment.media.length} media item(s), status ${response.status}`);
NODE

echo "agentic smoke passed"
