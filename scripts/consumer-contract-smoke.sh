#!/bin/sh
set -eu

OUT=.allie/consumer-contract-smoke
MANIFEST="$OUT/manifest.yml"
FIXTURE_DIR="../../fixtures/login"
VERIFY_CMD="allie verify --manifest .allie/manifest.yml --out .allie/verify/latest"

mkdir -p "$OUT"
rm -rf \
  "$MANIFEST" \
  "$OUT/discovery" \
  "$OUT/flow" \
  "$OUT/map" \
  "$OUT/run" \
  "$OUT/report" \
  "$OUT/release" \
  "$OUT/reporters"

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

const ciFiles = [
  'docs/ci/github-allie-verify.yml',
  'docs/ci/azure-pipelines-allie-verify.yml',
];
const github = fs.readFileSync(ciFiles[0], 'utf8');
const azure = fs.readFileSync(ciFiles[1], 'utf8');
for (const file of ciFiles) {
  const text = fs.readFileSync(file, 'utf8');
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
