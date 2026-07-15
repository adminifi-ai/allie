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
PUBLIC_OUT=.allie/consumer-publication-smoke
REFUSAL_OUT=.allie/consumer-publication-refusal-smoke

mkdir -p "$MANIFEST_DIR"
rm -rf "$MANIFEST" "$OUT" "$PUBLIC_OUT" "$REFUSAL_OUT"

cargo run --locked -- init \
  --manifest "$MANIFEST" \
  --app-name "Allie consumer fixture" \
  --fixture-dir "$FIXTURE_DIR" \
  --force

cargo run --locked -- verify \
  --manifest "$MANIFEST" \
  --out "$OUT" \
  --project-root fixtures/login

cargo run --locked -- publication \
  --verify-root "$OUT" \
  --out "$PUBLIC_OUT"

set +e
cargo run --locked -- publication \
  --verify-root "$OUT" \
  --out "$REFUSAL_OUT" \
  --include run/evidence.json
refusal_code=$?
set -e
test "$refusal_code" -eq 2

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
test -f "$PUBLIC_OUT/allie-public-summary.json"
test -f "$PUBLIC_OUT/allie-public-summary.md"
test -f "$PUBLIC_OUT/publication-receipt.json"
test -f "$PUBLIC_OUT/allie-run-manifest.json"
test -f "$REFUSAL_OUT/publication-receipt.json"
test -f "$OUT/run/evidence.json"
test ! -e "$PUBLIC_OUT/run/evidence.json"
test ! -e "$REFUSAL_OUT/run/evidence.json"

node - <<'NODE'
import fs from 'node:fs';
import { spawnSync } from 'node:child_process';

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

const publicSummary = JSON.parse(fs.readFileSync('.allie/consumer-publication-smoke/allie-public-summary.json', 'utf8'));
const publicReceipt = JSON.parse(fs.readFileSync('.allie/consumer-publication-smoke/publication-receipt.json', 'utf8'));
const refusalReceipt = JSON.parse(fs.readFileSync('.allie/consumer-publication-refusal-smoke/publication-receipt.json', 'utf8'));
const publicManifest = JSON.parse(fs.readFileSync('.allie/consumer-publication-smoke/allie-run-manifest.json', 'utf8'));
const refusalManifest = JSON.parse(fs.readFileSync('.allie/consumer-publication-refusal-smoke/allie-run-manifest.json', 'utf8'));
if (publicSummary.schema !== 'allie.public-summary.v0' || publicSummary.publication_class !== 'public_summary') {
  throw new Error('public projection must carry the public_summary contract');
}
if (JSON.stringify(publicSummary).includes(summary.project_root) || JSON.stringify(publicSummary).includes('run/evidence.json')) {
  throw new Error('public projection leaked a private path or canonical artifact reference');
}
if (publicReceipt.status !== 'ready' || publicReceipt.retryable !== false) {
  throw new Error('approved public projection must emit a ready, non-retryable receipt');
}
if (publicReceipt.publication_class !== 'public_summary' || refusalReceipt.publication_class !== 'public_summary') {
  throw new Error('every publicly uploaded publication receipt must classify itself as public_summary');
}
if (publicManifest.publication_class !== 'public_summary' || refusalManifest.publication_class !== 'public_summary') {
  throw new Error('every publicly uploaded publication run manifest must classify itself as public_summary');
}
if (!publicReceipt.published.every((item) => item.publication_class === 'public_summary')) {
  throw new Error('public publisher accepted a non-public_summary artifact');
}
if (refusalReceipt.status !== 'refused' || refusalReceipt.retryable !== true) {
  throw new Error('raw evidence request must emit a retryable refusal receipt');
}
if (refusalReceipt.refused[0]?.publication_class !== 'sensitive_local') {
  throw new Error('raw evidence refusal must name its sensitive_local class');
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
const publicationCommand = 'allie publication --verify-root .allie/verify/latest --out .allie/public/latest';
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
  if (!text.includes(publicationCommand)) {
    throw new Error(`${file} does not prepare the policy-approved public projection`);
  }
  const forbidden = /\ballie\s+(run|discover|promote-flow|map|report|release|workbench|review|remediate|init)\b/;
  if (forbidden.test(text)) {
    throw new Error(`${file} must not call lower-level Allie commands`);
  }
}
const publicFiles = [
  'allie-public-summary.json',
  'allie-public-summary.md',
  'publication-receipt.json',
  'allie-run-manifest.json',
];
if (/(^|\n)\s*path:\s*\.allie\/public\/latest\s*($|\n)/.test(github) || github.includes('path: .allie/verify/latest')) {
  throw new Error('GitHub example must not upload an uncontrolled output directory');
}
for (const file of publicFiles) {
  if (!github.includes(`.allie/public/latest/${file}`)) {
    throw new Error(`GitHub example does not explicitly allowlist ${file}`);
  }
  if (!azure.includes(`        ${file}`)) {
    throw new Error(`Azure example does not explicitly allowlist ${file}`);
  }
}
const githubPublicFiles = [...github.matchAll(/^\s+\.allie\/public\/latest\/(\S+)$/gm)]
  .map((match) => match[1]);
if (JSON.stringify(githubPublicFiles) !== JSON.stringify(publicFiles)) {
  throw new Error(`GitHub publication allowlist must be exact, got ${githubPublicFiles}`);
}
const azureContents = azure.match(/Contents: \|\n([\s\S]*?)\n\s+TargetFolder:/)?.[1]
  .trim()
  .split(/\n/)
  .map((line) => line.trim());
if (JSON.stringify(azureContents) !== JSON.stringify(publicFiles)) {
  throw new Error(`Azure publication allowlist must be exact, got ${azureContents}`);
}
if (!azure.includes('SourceFolder: .allie/public/latest') || !azure.includes('publish: $(Build.ArtifactStagingDirectory)/allie-public') || azure.includes('publish: .allie/verify/latest')) {
  throw new Error('Azure example must stage only allowlisted files before publication');
}
if (!github.includes("if: always() && steps.publication.outcome == 'success'")) {
  throw new Error('GitHub upload must run only after successful public projection');
}
if (!azure.includes('##vso[task.setvariable variable=ALLIE_PUBLICATION_READY]true')) {
  throw new Error('Azure publication must emit its success gate only after projection succeeds');
}
const azurePublicationDisplay = '\n    displayName: Prepare policy-approved public summary';
const azurePublicationEnd = azure.indexOf(azurePublicationDisplay);
const azurePublicationStart = azure.lastIndexOf('  - script: |\n', azurePublicationEnd);
const azurePublication = azure
  .slice(azurePublicationStart + '  - script: |\n'.length, azurePublicationEnd)
  .split(/\n/)
  .map((line) => line.trim())
  .filter(Boolean)
  .join('\n');
if (!azurePublication?.startsWith('set -eu\n')) {
  throw new Error('Azure publication must fail before setting readiness');
}
const failedAzurePublication = spawnSync(
  'sh',
  ['-c', azurePublication.replace(publicationCommand, 'false')],
  { encoding: 'utf8' },
);
if (
  failedAzurePublication.status === 0
  || !failedAzurePublication.stdout.includes('ALLIE_PUBLICATION_READY]false')
  || failedAzurePublication.stdout.includes('ALLIE_PUBLICATION_READY]true')
) {
  throw new Error('Azure publication failure must reset readiness and never set it true');
}
if ((azure.match(/condition: and\(always\(\), eq\(variables\['ALLIE_PUBLICATION_READY'\], 'true'\)\)/g) || []).length !== 2) {
  throw new Error('Azure staging and publication must require successful public projection');
}

const command = 'allie verify --manifest .allie/manifest.yml --out .allie/verify/latest';
if ((github.match(new RegExp(command, 'g')) || []).length !== 1) {
  throw new Error('GitHub example must call allie verify exactly once');
}
if ((azure.match(new RegExp(command, 'g')) || []).length !== 1) {
  throw new Error('Azure example must call allie verify exactly once');
}
if ((github.match(new RegExp(publicationCommand, 'g')) || []).length !== 1) {
  throw new Error('GitHub example must call allie publication exactly once');
}
if ((azure.match(new RegExp(publicationCommand, 'g')) || []).length !== 1) {
  throw new Error('Azure example must call allie publication exactly once');
}
NODE

echo "consumer contract smoke passed: $OUT"
