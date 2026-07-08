#!/bin/sh
# AL-085: prove the five axe-core rules named in
# docs/criteria-assessability-research.md (target-size,
# label-content-name-mismatch, autocomplete-valid, css-orientation-lock,
# meta-viewport) are caught deterministically against a fixture with known
# violations, with the model disabled (model.enabled: false in the manifest).
# Also guards against the specific false-negative that kept 2.5.3 Label in
# Name on method: human_review (see the second run below).
set -eu

RUN_DIR=.allie/runs/axe-named-rules-smoke
NEGATING_LABEL_RUN_DIR=.allie/runs/axe-negating-label-in-name-smoke
TARGET_SIZE_PASS_RUN_DIR=.allie/runs/axe-target-size-pass-smoke
TARGET_SIZE_EMPTY_RUN_DIR=.allie/runs/axe-target-size-empty-smoke

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

// Regression guard: the desktop pass and the dedicated mobile-viewport pass
// each run axe independently, and a fixture defect that isn't viewport-
// dependent (all five of this fixture's defects render identically at both
// widths) previously produced TWO findings per defect — one orphaned by the
// verdict layer's per-obligation BTreeMap collapse. findingsFor returns every
// match (not just the first) so an exactly-one-per-rule assertion below can
// catch that duplication if it regresses.
function findingsFor(axeRuleId) {
  return packet.findings.filter((finding) => finding.id.includes(`-axe-${axeRuleId}-`));
}

// Each named rule must have produced exactly one deterministic, machine-proven
// finding (not silently dropped, not duplicated across viewports, and not
// routed through any model/agentic path).
const expectedRules = [
  'target-size',
  'label-content-name-mismatch',
  'autocomplete-valid',
  'css-orientation-lock',
  'meta-viewport',
];
for (const ruleId of expectedRules) {
  const findings = findingsFor(ruleId);
  assert(
    findings.length === 1,
    `expected exactly one finding for axe rule ${ruleId}, got ${findings.length} (desktop/mobile dedup may have regressed)`,
  );
  const [finding] = findings;
  assert(finding.status === 'fail', `${ruleId} finding must be a fail, got ${finding.status}`);
  assert(finding.confidence === 'machine_proven', `${ruleId} finding must be machine_proven, got ${finding.confidence}`);
  assert(finding.evidence_class === 'deterministic', `${ruleId} finding must be deterministic, got ${finding.evidence_class}`);
  assert(finding.source === 'axe-core', `${ruleId} finding must be sourced from axe-core, got ${finding.source}`);
}

// The fixture carries exactly five deliberate defects (one per named rule);
// any count other than 5 means either a missed rule or a duplicate/orphan
// finding slipped past the per-rule check above.
assert(
  packet.findings.length === 5,
  `expected exactly 5 findings on the fixture (one per named rule), got ${packet.findings.length}`,
);

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
  const [finding] = findingsFor(ruleId);
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

# Regression guard for the reviewer-found false-negative: axe-core's
# label-content-name-mismatch rule is a substring-containment check, not a
# semantic one, so a negating accessible name that still contains the
# visible text (aria-label="Do not cancel" on a button reading "Cancel")
# reports zero violations. This run is expected to pass cleanly at the axe
# layer; the assertion below is that WCAG 2.5.3 Label in Name must NOT come
# back as a machine-proven pass on the strength of that silence.
rm -rf "$NEGATING_LABEL_RUN_DIR"
cargo run --locked -- run --manifest examples/axe-negating-label-in-name-flow.yml --out "$NEGATING_LABEL_RUN_DIR"

node - "$NEGATING_LABEL_RUN_DIR/evidence.json" <<'NODE'
const fs = require('fs');
const packet = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

const labelInName = packet.verdicts.find((item) => item.obligation === 'wcag22-aa:2.5.3-label-in-name');
assert(labelInName, 'expected a verdict for wcag22-aa:2.5.3-label-in-name');
assert(
  !(labelInName.status === 'pass' && labelInName.confidence === 'machine_proven'),
  'wcag22-aa:2.5.3-label-in-name must not resolve to a machine-proven pass on a negating accessible name axe cannot detect; method must stay human_review in profiles/wcag22-aa.json',
);
assert(
  labelInName.status === 'needs_review',
  `expected wcag22-aa:2.5.3-label-in-name to route to needs_review, got ${labelInName.status}`,
);

console.log('axe negating-label-in-name regression guard passed: 2.5.3 stayed needs_review, not a false machine-proven pass');
NODE

rm -rf "$TARGET_SIZE_PASS_RUN_DIR"
cargo run --locked -- run --manifest examples/axe-target-size-pass-flow.yml --out "$TARGET_SIZE_PASS_RUN_DIR"

node - "$TARGET_SIZE_PASS_RUN_DIR/evidence.json" <<'NODE'
const fs = require('fs');
const packet = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

const targetSize = packet.verdicts.find((item) => item.obligation === 'wcag22-aa:2.5.8-target-size-minimum');
assert(targetSize, 'expected a verdict for wcag22-aa:2.5.8-target-size-minimum');
assert(
  targetSize.status === 'pass',
  `expected target-size to pass when axe evaluated a matching control, got ${targetSize.status}`,
);
assert(
  targetSize.confidence === 'machine_proven',
  `target-size pass must be machine_proven, got ${targetSize.confidence}`,
);
assert(
  targetSize.source === 'axe-core:target-size',
  `target-size pass must identify the trusted axe rule source, got ${targetSize.source}`,
);
assert(
  targetSize.affected_states.includes('target-size-pass'),
  'target-size pass must be scoped to the state where axe evaluated matching controls',
);

console.log('axe target-size pass guard passed: evaluated matching controls can produce a machine pass');
NODE

rm -rf "$TARGET_SIZE_EMPTY_RUN_DIR"
cargo run --locked -- run --manifest examples/axe-target-size-empty-flow.yml --out "$TARGET_SIZE_EMPTY_RUN_DIR"

node - "$TARGET_SIZE_EMPTY_RUN_DIR/evidence.json" <<'NODE'
const fs = require('fs');
const packet = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

const targetSize = packet.verdicts.find((item) => item.obligation === 'wcag22-aa:2.5.8-target-size-minimum');
assert(targetSize, 'expected a verdict for wcag22-aa:2.5.8-target-size-minimum');
assert(
  targetSize.status === 'needs_review',
  `expected target-size with no matching axe evaluation to stay needs_review, got ${targetSize.status}`,
);
assert(
  targetSize.source === 'allie-mobile-web-viewport-audit',
  `target-size empty evaluation must keep mobile evidence attached, got source ${targetSize.source}`,
);
assert(
  packet.artifacts.some((artifact) => artifact.id === 'mobile-screenshot-target-size-empty'),
  'target-size empty run must retain mobile screenshot evidence',
);

console.log('axe target-size empty guard passed: no matching elements stayed needs_review with mobile evidence');
NODE

echo "axe-named-rules smoke passed: $RUN_DIR/evidence.json, $NEGATING_LABEL_RUN_DIR/evidence.json, $TARGET_SIZE_PASS_RUN_DIR/evidence.json, and $TARGET_SIZE_EMPTY_RUN_DIR/evidence.json"
