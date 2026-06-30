#!/bin/sh
# State action-step smoke (ticket 015, child 1).
#
# Proves `flow.states[].steps` run after navigation and before evidence capture:
# the workbench fixture menu starts hidden, the manifest clicks it open, and the
# DOM artifact must contain the revealed settings link without the hidden attr.
set -eu

OUT=.allie/runs/action-steps-smoke
NEG_OUT=.allie/runs/action-steps-negative-smoke
rm -rf "$OUT" "$NEG_OUT"

cargo run --locked -- run \
  --manifest examples/action-steps-flow.yml \
  --out "$OUT"

OUT="$OUT" node -e "
const fs = require('fs');
const path = require('path');
const packet = JSON.parse(fs.readFileSync(path.join(process.env.OUT, 'evidence.json'), 'utf8'));
if (!['pass', 'approved', 'passed'].includes(packet.summary.status)) {
  throw new Error('action-step packet status not pass/approved: ' + packet.summary.status);
}
const state = packet.coverage.state_metadata.find((item) => item.id === 'open-menu');
if (!state) throw new Error('missing open-menu state');
if (state.state_errors.length !== 0) {
  throw new Error('state action errors were recorded: ' + JSON.stringify(state.state_errors));
}
const typed = packet.coverage.state_metadata.find((item) => item.id === 'typed-email');
if (!typed) throw new Error('missing typed-email state');
if (typed.state_errors.length !== 0) {
  throw new Error('typed-email action errors were recorded: ' + JSON.stringify(typed.state_errors));
}
"

DOM="$OUT/artifacts/dom-open-menu.html"
test -f "$DOM"
grep -q 'id=\"menu\"' "$DOM"
grep -q 'Manage account settings' "$DOM"
if grep -q 'id=\"menu\" hidden' "$DOM"; then
  echo "FAIL: menu stayed hidden in DOM artifact"
  exit 1
fi
TYPED_DOM="$OUT/artifacts/dom-typed-email.html"
test -f "$TYPED_DOM"
grep -q 'qa@example.test.typed' "$TYPED_DOM"
grep -q 'id=\"email-preview\"' "$TYPED_DOM"
grep -q '"steps"' "$OUT/worker-request.json"

set +e
cargo run --locked -- run \
  --manifest examples/action-steps-negative-flow.yml \
  --out "$NEG_OUT"
neg_status=$?
set -e
if [ "$neg_status" -eq 0 ]; then
  echo "FAIL: action-step negative control passed but must block"
  exit 1
fi

NEG_OUT="$NEG_OUT" node -e "
const fs = require('fs');
const path = require('path');
const packet = JSON.parse(fs.readFileSync(path.join(process.env.NEG_OUT, 'evidence.json'), 'utf8'));
if (['pass', 'approved', 'passed'].includes(packet.summary.status)) {
  throw new Error('negative action-step packet must not pass: ' + packet.summary.status);
}
const state = packet.coverage.state_metadata.find((item) => item.id === 'missing-action-target');
if (!state) throw new Error('missing negative-control state');
if (!state.state_errors.some((error) => error.includes('state-step-failed'))) {
  throw new Error('missing state-step-failed error: ' + JSON.stringify(state.state_errors));
}
"

echo "action steps smoke passed: $OUT and negative control blocked"
