#!/bin/sh
# V0 evidence smoke: run the checked-in fixture, prove important packet content,
# and ensure frozen-clock reruns are byte-stable for packet + HTML report.
set -eu

RUN_DIR=.allie/runs/v0-smoke
FREEZE_EPOCH=1700000000
FREEZE_RFC3339="2023-11-14T22:13:20+00:00"
FREEZE_PORT=43100
FIRST="$(mktemp -d)"
cleanup() {
  rm -rf "$FIRST"
}
trap cleanup EXIT

run_fixture() {
  rm -rf "$RUN_DIR"
  ALLIE_FIXTURE_PORT="$FREEZE_PORT" \
  SOURCE_DATE_EPOCH="$FREEZE_EPOCH" \
    cargo run --locked -- run --manifest examples/login-flow.yml --out "$RUN_DIR"
}

run_fixture
cp "$RUN_DIR/evidence.json" "$FIRST/evidence.json"
cp "$RUN_DIR/report.html" "$FIRST/report.html"

run_fixture
cmp "$FIRST/evidence.json" "$RUN_DIR/evidence.json"
cmp "$FIRST/report.html" "$RUN_DIR/report.html"

node - "$RUN_DIR/evidence.json" "$RUN_DIR/report.html" "$FREEZE_RFC3339" "$FREEZE_PORT" <<'NODE'
import fs from 'node:fs';

const [packetPath, reportPath, freezeTime, freezePort] = process.argv.slice(2);
const packet = JSON.parse(fs.readFileSync(packetPath, 'utf8'));
const report = fs.readFileSync(reportPath, 'utf8');

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function verdict(obligation) {
  return packet.verdicts.find((item) => item.obligation === obligation);
}

assert(packet.schema === 'allie.evidence.v0', `unexpected schema ${packet.schema}`);
assert(packet.summary.status === 'pass', `unexpected status ${packet.summary.status}`);
assert(packet.summary.exit_code === 0, `unexpected exit code ${packet.summary.exit_code}`);
assert(packet.summary.states_captured === 1, 'expected one captured state');
assert(packet.summary.infrastructure_failures === 0, 'expected zero infrastructure failures');
assert(packet.run.id === 'run-1700000000000', `unexpected run id ${packet.run.id}`);
assert(packet.run.started_at === freezeTime, `unexpected started_at ${packet.run.started_at}`);
assert(packet.run.finished_at === freezeTime, `unexpected finished_at ${packet.run.finished_at}`);
assert(packet.run.git_sha && packet.run.git_sha !== 'unknown', 'git_sha must be present');
assert(packet.run.git_branch && packet.run.git_branch !== 'unknown', 'git_branch must be present');
assert(packet.target.base_url === `http://127.0.0.1:${freezePort}/`, `unexpected base_url ${packet.target.base_url}`);
assert(packet.coverage.routes_visited.includes('/'), 'coverage must include fixture route');
assert(packet.coverage.states_captured.includes('login-form'), 'coverage must include login-form state');
assert(packet.coverage.state_metadata.some((state) => (
  state.id === 'login-form'
  && state.url === `http://127.0.0.1:${freezePort}/`
  && state.http_status === 200
  && state.features?.reflow_checked === true
)), 'login-form state metadata must be contentful');
assert(packet.artifacts.some((artifact) => artifact.id === 'axe-json-login-form' && artifact.hash?.startsWith('sha256:')), 'axe artifact hash missing');
assert(packet.artifacts.some((artifact) => artifact.id === 'screenshot-login-form' && artifact.hash?.startsWith('sha256:')), 'screenshot artifact hash missing');

const deterministic = verdict('wcag22-aa:deterministic-axe-rules');
assert(deterministic?.status === 'pass', 'deterministic aggregate must pass');
assert(deterministic?.confidence === 'machine_proven', 'deterministic confidence must stay distinct');
const reflow = verdict('wcag22-aa:1.4.10-reflow');
assert(reflow?.status === 'pass', 'reflow scripted verdict must pass in fixture');
assert(reflow?.confidence === 'script_observed', 'scripted confidence must stay distinct');
assert(packet.verdicts.some((item) => item.status === 'needs_review'), 'fixture must retain explicit review obligations');
assert(packet.verdicts.some((item) => item.status === 'not_applicable'), 'fixture must retain not_applicable obligations');

assert(report.includes('Allie evidence status'), 'report missing evidence status');
assert(report.includes('No deterministic axe failures'), 'report missing deterministic pass summary');
assert(report.includes('<h2>Replay</h2>'), 'report missing replay section');
NODE

test -f "$RUN_DIR/evidence.json"
test -f "$RUN_DIR/report.html"
test -f "$RUN_DIR/artifacts/axe-login-form.json"
test -f "$RUN_DIR/artifacts/login-form.png"
echo "evidence smoke passed: byte-stable frozen packet/report with content assertions"
