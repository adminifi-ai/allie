#!/bin/sh
set -eu

# Post-AL-082, `allie init` auto-enables model review when a provider API key
# resolves in the environment. This smoke calls `init` then `verify` and is
# meant to be offline-deterministic, so isolate it from any ambient keys
# rather than making real, billed model calls whenever a developer's shell
# happens to export one.
unset OPENROUTER_API_KEY OPENAI_API_KEY

OUT=.allie/consumer-contract-smoke
# The manifest lives beside --out, not inside it, matching the documented
# consumer layout (docs/ci/github-allie-verify.yml: .allie/manifest.yml next
# to --out .allie/verify/latest as two separate paths). Allie's own out-dir
# hygiene (AL-117) refuses a --out directory that already has files it did
# not write and cannot account for via its run manifest, so a flow manifest
# colocated inside --out would trip that refusal on every rerun.
MANIFEST_DIR=.allie/consumer-contract-smoke-config
MANIFEST="$MANIFEST_DIR/manifest.yml"
FIXTURE_DIR="../../fixtures/login"
VERIFY_CMD="allie verify --manifest .allie/manifest.yml --out .allie/verify/latest"

mkdir -p "$MANIFEST_DIR"
rm -rf "$MANIFEST" "$OUT"

cargo run --locked -- init \
  --manifest "$MANIFEST" \
  --app-name "Allie consumer fixture" \
  --fixture-dir "$FIXTURE_DIR" \
  --force

cargo run --locked -- verify \
  --manifest "$MANIFEST" \
  --out "$OUT" \
  --project-root fixtures/login

test -f "$MANIFEST"
test -f "$OUT/discovery/discovery.json"
test -f "$OUT/discovery/flow-plan.json"
test -f "$OUT/flow/generated-flow.yml"
test -f "$OUT/map/product-map.json"
test -f "$OUT/map/surface-map.html"
test -f "$OUT/map/agent-runner-receipt.json"
test -f "$OUT/run/evidence.json"
test -f "$OUT/run/report.html"
test -f "$OUT/report/compliance-report.json"
test -f "$OUT/report/compliance-report.html"
test -f "$OUT/report/summary.md"
test -f "$OUT/release/release-summary.json"
test -f "$OUT/release/github-check.json"
test -f "$OUT/release/release-report.html"
test -f "$OUT/reporters/allie-report.json"
test -f "$OUT/reporters/allie-compliance-report.json"
test -f "$OUT/reporters/allie-report.html"
test -f "$OUT/reporters/allie-report.md"
test -f "$OUT/reporters/junit.xml"
test -f "$OUT/reporters/allie.sarif"

node - <<'NODE'
import fs from 'node:fs';

const summary = JSON.parse(fs.readFileSync('.allie/consumer-contract-smoke/reporters/allie-report.json', 'utf8'));
if (summary.schema !== 'allie.verify.v0') {
  throw new Error(`unexpected verify schema ${summary.schema}`);
}
if (summary.host_agnostic !== true) {
  throw new Error('verify summary must be host agnostic');
}
for (const format of ['json', 'html', 'markdown', 'junit', 'sarif']) {
  if (!summary.reporters[format]) {
    throw new Error(`missing reporter ${format}`);
  }
}
const expectedReporters = {
  json: 'reporters/allie-report.json',
  wcag_json: 'reporters/allie-compliance-report.json',
  html: 'reporters/allie-report.html',
  markdown: 'reporters/allie-report.md',
  junit: 'reporters/junit.xml',
  sarif: 'reporters/allie.sarif',
};
for (const [key, value] of Object.entries(expectedReporters)) {
  if (summary.reporters[key] !== value) {
    throw new Error(`reporter ${key} expected ${value}, got ${summary.reporters[key]}`);
  }
}
const wcag = JSON.parse(fs.readFileSync('.allie/consumer-contract-smoke/reporters/allie-compliance-report.json', 'utf8'));
if (wcag.schema !== 'allie.compliance-report.v0') {
  throw new Error(`unexpected WCAG report schema ${wcag.schema}`);
}
if (JSON.stringify(summary).toLowerCase().includes('is a legal compliance guarantee')) {
  throw new Error('summary must not claim legal compliance');
}
const html = fs.readFileSync('.allie/consumer-contract-smoke/reporters/allie-report.html', 'utf8');
for (const [, href] of html.matchAll(/href="([^"]+)"/g)) {
  if (href.startsWith('http:') || href.startsWith('https:') || href.startsWith('#')) {
    continue;
  }
  const resolved = new URL(href, 'file://' + process.cwd() + '/.allie/consumer-contract-smoke/reporters/allie-report.html');
  if (!fs.existsSync(resolved)) {
    throw new Error(`report HTML link is not closed under verify artifact root: ${href}`);
  }
}

// AL-123: review grain labels — a reader of allie-report.md/.html must tell
// why two "review" numbers differ without opening source. Each grain carries
// a one-line label at the point of print; all three appear once in a single
// reconciled "what still needs review and why" block.
const markdown = fs.readFileSync('.allie/consumer-contract-smoke/reporters/allie-report.md', 'utf8');
const review = summary.why && summary.why.review;
if (!review) {
  throw new Error('allie-report.json must surface why.review with the three grains');
}
const grains = [
  ['verdict_review_needed_obligations', 'verdict', 'Verdict-grain'],
  ['criteria_needs_review', 'criterion', 'Criterion-grain'],
  ['profile_human_review_scope', 'profile', 'Profile-scope'],
];
for (const [key, grain, namePrefix] of grains) {
  const entry = review[key];
  if (!entry || typeof entry.count !== 'number') {
    throw new Error(`why.review.${key} must carry a numeric count`);
  }
  if (entry.grain !== grain) {
    throw new Error(`why.review.${key}.grain expected ${grain}, got ${entry.grain}`);
  }
  if (typeof entry.label !== 'string' || !entry.label.includes(namePrefix.split('-')[0])) {
    throw new Error(`why.review.${key}.label must be a one-line ${namePrefix} description, got ${entry.label}`);
  }
  if (!markdown.includes(entry.label)) {
    throw new Error(`allie-report.md must print the ${key} grain label at the point of print`);
  }
  if (!html.includes(entry.label)) {
    throw new Error(`allie-report.html must print the ${key} grain label at the point of print`);
  }
}
if (!markdown.includes('Review scope — what still needs review, and why')) {
  throw new Error('allie-report.md must carry a single reconciled review-scope block');
}
if (!html.includes('Review scope — what still needs review, and why')) {
  throw new Error('allie-report.html must carry a single reconciled review-scope block');
}
if (!html.includes('Review needed (verdict-grain)')) {
  throw new Error('allie-report.html blocking tile must label verdict-grain, not bare "Review needed"');
}
if (!html.includes('Needs review (criterion-grain)')) {
  throw new Error('allie-report.html WCAG tile must label criterion-grain, not bare "Needs review"');
}
if (/\bReview needed\b(?! \(verdict-grain\))/.test(html.replace(/Review needed \(verdict-grain\)/g, ''))) {
  throw new Error('allie-report.html must not print a bare "Review needed" tile without a grain label');
}

const ciFiles = [
  'docs/ci/github-allie-verify.yml',
  'docs/ci/azure-pipelines-allie-verify.yml',
];
const github = fs.readFileSync(ciFiles[0], 'utf8');
const azure = fs.readFileSync(ciFiles[1], 'utf8');
const doctorCommand = 'allie doctor --manifest .allie/manifest.yml --out .allie/doctor';
for (const file of ciFiles) {
  const text = fs.readFileSync(file, 'utf8');
  if (text.includes('ALLIE_BROWSER_WORKER')) {
    throw new Error(`${file} must not require ALLIE_BROWSER_WORKER for the normal consumer path`);
  }
  if (text.includes('cargo install') || text.includes('git clone')) {
    throw new Error(`${file} must consume the prebuilt Allie bundle instead of compiling in every consumer run`);
  }
  if (!text.includes(doctorCommand)) {
    throw new Error(`${file} does not run allie doctor before verify`);
  }
  if (!text.includes('allie verify --manifest .allie/manifest.yml --out .allie/verify/latest')) {
    throw new Error(`${file} does not call the portable verify command`);
  }
  if (!text.includes('.allie/verify/latest')) {
    throw new Error(`${file} does not publish the full verify artifact root`);
  }
  const forbidden = /\ballie\s+(run|discover|promote-flow|map|report|release|workbench|review|remediate|init)\b/;
  if (forbidden.test(text)) {
    throw new Error(`${file} must not call lower-level Allie commands`);
  }
}
if (!github.includes('if: always()')) {
  throw new Error('GitHub example must upload evidence even when verify blocks');
}
if (!azure.includes('condition: always()')) {
  throw new Error('Azure example must publish evidence even when verify blocks');
}

const command = 'allie verify --manifest .allie/manifest.yml --out .allie/verify/latest';
if ((github.match(new RegExp(command, 'g')) || []).length !== 1) {
  throw new Error('GitHub example must call allie verify exactly once');
}
if ((azure.match(new RegExp(command, 'g')) || []).length !== 1) {
  throw new Error('Azure example must call allie verify exactly once');
}
NODE

echo "consumer contract smoke passed: $OUT"
