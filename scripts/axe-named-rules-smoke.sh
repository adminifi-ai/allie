#!/bin/sh
# AL-085: prove the five axe-core rules named in
# docs/criteria-assessability-research.md (target-size,
# label-content-name-mismatch, autocomplete-valid, css-orientation-lock,
# meta-viewport) are caught deterministically against a fixture with known
# violations, with the model disabled (model.enabled: false in the manifest).
set -eu

RUN_DIR=.allie/runs/axe-named-rules-smoke

rm -rf "$RUN_DIR"

# The fixture deliberately violates five axe rules, so this run is expected to
# report a blocking finding (nonzero exit); disable errexit around it and
# assert the exit code explicitly instead of letting it abort the script.
set +e
cargo run --locked -- run --manifest examples/axe-named-rules-flow.yml --out "$RUN_DIR"
run_status=$?
set -e
if [ "$run_status" -eq 0 ]; then
  echo "FAIL: axe-named-rules fixture run passed but must report deterministic violations"
  exit 1
fi

node - "$RUN_DIR/evidence.json" <<'NODE'
const fs = require('fs');
const packet = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function verdict(obligation) {
  return packet.verdicts.find((item) => item.obligation === obligation);
}

function findingFor(axeRuleId) {
  return packet.findings.find((finding) => finding.id.includes(`-axe-${axeRuleId}-`));
}

// Each named rule must have produced a deterministic, machine-proven finding
// (not silently dropped, and not routed through any model/agentic path).
const expectedRules = [
  'target-size',
  'label-content-name-mismatch',
  'autocomplete-valid',
  'css-orientation-lock',
  'meta-viewport',
];
for (const ruleId of expectedRules) {
  const finding = findingFor(ruleId);
  assert(finding, `expected a finding for axe rule ${ruleId}`);
  assert(finding.status === 'fail', `${ruleId} finding must be a fail, got ${finding.status}`);
  assert(finding.confidence === 'machine_proven', `${ruleId} finding must be machine_proven, got ${finding.confidence}`);
  assert(finding.evidence_class === 'deterministic', `${ruleId} finding must be deterministic, got ${finding.evidence_class}`);
  assert(finding.source === 'axe-core', `${ruleId} finding must be sourced from axe-core, got ${finding.source}`);
}

// Each rule's obligation mapping must resolve to the correct WCAG success
// criterion (via profiles/wcag22-aa.json axe_tag_map), not the generic
// unmapped-axe-rule bucket, and the corresponding per-criterion verdict must
// be a deterministic fail.
const expectedObligations = {
  'wcag22-aa:2.5.3-label-in-name': 'label-content-name-mismatch',
  'wcag22-aa:2.5.8-target-size-minimum': 'target-size',
  'wcag22-aa:1.3.5-identify-input-purpose': 'autocomplete-valid',
  'wcag22-aa:1.3.4-orientation': 'css-orientation-lock',
  'wcag22-aa:1.4.4-resize-text': 'meta-viewport',
};
for (const [obligation, ruleId] of Object.entries(expectedObligations)) {
  const finding = findingFor(ruleId);
  assert(
    finding.standard_obligation === obligation,
    `${ruleId} must map to ${obligation}, got ${finding.standard_obligation}`,
  );
  const criterionVerdict = verdict(obligation);
  assert(criterionVerdict, `expected a per-criterion verdict for ${obligation}`);
  assert(criterionVerdict.status === 'fail', `${obligation} verdict must be fail, got ${criterionVerdict.status}`);
  assert(
    criterionVerdict.confidence === 'machine_proven',
    `${obligation} verdict must be machine_proven, got ${criterionVerdict.confidence}`,
  );
}

assert(
  !packet.findings.some((finding) => finding.standard_obligation === 'wcag22-aa:unmapped-axe-rule'),
  'none of the five named rules should fall into the unmapped-axe-rule bucket',
);

assert(packet.policy.profile === 'wcag22-aa', 'expected the wcag22-aa policy profile');

console.log('axe named-rules smoke passed: all five rules mapped to their WCAG obligation as deterministic fails');
NODE

test -f "$RUN_DIR/evidence.json"
echo "axe-named-rules smoke passed: $RUN_DIR/evidence.json"
