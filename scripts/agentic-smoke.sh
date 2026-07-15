#!/bin/sh
# Deterministic guard for the agentic review gateway.
#
# Runs the gateway against the bundled login fixture in two offline modes:
# first with NO model API key to prove graceful degradation, then with a local
# fake OpenRouter endpoint to prove the model payload includes screenshots and
# captured video clips without touching the network.
set -eu

ALLIE_REPO="$(pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

cat > "$WORK/request.json" <<JSON
{
  "schema": "allie.agentic.request.v0",
  "target": { "fixture_dir": "$ALLIE_REPO/fixtures/login" },
  "browser": { "viewport": { "width": 1024, "height": 768 }, "color_scheme": "light", "reduced_motion": "reduce", "locale": "en-US" },
  "model": { "provider": "openrouter", "model": "offline", "api_key_env": "ALLIE_AGENTIC_SMOKE_NO_KEY", "base_url": "https://openrouter.ai/api/v1", "max_calls": 1, "redaction": "none" },
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
if (assessment.verdict !== 'inconclusive') {
  throw new Error(`expected verdict "inconclusive" without an API key (never a fabricated pass/fail), got "${assessment.verdict}"`);
}
if (assessment.confidence !== 'not_observed') {
  throw new Error(`expected confidence "not_observed" without an API key, got "${assessment.confidence}"`);
}
if (!assessment.media.some((entry) => entry.kind === 'screenshot')) {
  throw new Error('gateway did not capture screenshot evidence');
}
if (JSON.stringify(response.redaction_receipt) !== JSON.stringify({
  schema: 'allie.model-redaction-receipt.v0',
  profile: 'none',
  status: 'not_sent',
})) {
  throw new Error(`offline response did not carry a truthful not_sent redaction receipt: ${JSON.stringify(response.redaction_receipt)}`);
}
console.log(`agentic smoke ok: captured ${assessment.media.length} media item(s), status ${response.status}`);
NODE

node scripts/agentic-video-payload-smoke.mjs "$WORK" "$ALLIE_REPO"

echo "agentic smoke passed"
