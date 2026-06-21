#!/bin/sh
set -eu

RUN_DIR=.allie/runs/coverage-matrix-smoke
REPORT_DIR=.allie/reports/coverage-matrix-smoke

rm -rf "$RUN_DIR" "$REPORT_DIR"
mkdir -p "$RUN_DIR"
cp fixtures/vanity-dogfood-legacy-61/evidence.json "$RUN_DIR/evidence.json"

cargo run --locked -- report \
  --map fixtures/vanity-dogfood-legacy-61/product-map.json \
  --packet "$RUN_DIR/evidence.json" \
  --out "$REPORT_DIR"

REPORT_DIR="$REPORT_DIR" node -e "
const fs = require('fs');
const report = JSON.parse(fs.readFileSync(process.env.REPORT_DIR + '/compliance-report.json', 'utf8'));
const html = fs.readFileSync(process.env.REPORT_DIR + '/compliance-report.html', 'utf8');
const supportIds = [
  'wcag22-aa:deterministic-axe-rules',
  'wcag22-aa:2.1.1-keyboard-traversal',
  'wcag22-aa:1.4.10-zoom-reflow',
  'wcag22-aa:2.2.2-reduced-motion',
  'wcag22-aa:human-content-meaning',
  'wcag22-aa:human-assistive-technology-review',
];
if (report.summary.total_obligations !== 55) throw new Error('expected 55 WCAG obligations');
if (report.summary.total_success_criteria !== 55) throw new Error('expected 55 success criteria');
if (report.summary.total_supporting_checks !== 6) throw new Error('expected 6 supporting checks');
if (report.criteria.length !== 55) throw new Error('criteria length must be 55');
if (report.criterion_coverage.length !== 55) throw new Error('coverage matrix length must be 55 for one fixture state');
const criterionIds = new Set(report.criteria.map((criterion) => criterion.id));
for (const id of supportIds) {
  if (criterionIds.has(id)) throw new Error(id + ' leaked into WCAG denominator');
  if (!report.supporting_checks.some((check) => check.id === id)) throw new Error(id + ' missing from support checks');
}
for (const cell of report.criterion_coverage) {
  for (const key of ['status', 'applicability', 'method', 'confidence', 'evidence_refs', 'agentic_refs', 'waiver_refs', 'residual_review_need']) {
    if (!(key in cell)) throw new Error('coverage cell missing ' + key);
  }
  if (['pass', 'fail', 'waived', 'risk_accepted'].includes(cell.status)) {
    const refs = ['evidence_refs', 'agentic_refs', 'waiver_refs', 'finding_refs', 'artifact_refs', 'test_refs']
      .some((key) => Array.isArray(cell[key]) && cell[key].length > 0);
    if (!refs && !cell.replay_command) throw new Error('terminal cell lacks provenance: ' + cell.criterion_id);
  }
}
if (!html.includes('WCAG 2.2 success criteria')) throw new Error('html missing criteria section');
if (!html.includes('Supporting checks')) throw new Error('html missing support section');
if (!html.includes('Criterion coverage matrix')) throw new Error('html missing coverage matrix');
if (!html.includes('not a legal compliance guarantee')) throw new Error('html missing no-legal-claim text');
"

test -f "$RUN_DIR/evidence.json"
test -f "$REPORT_DIR/compliance-report.json"
test -f "$REPORT_DIR/compliance-report.html"
test -f "$REPORT_DIR/summary.md"
