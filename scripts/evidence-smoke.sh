#!/bin/sh
# V0 evidence smoke: run the checked-in fixture, prove important packet content,
# and ensure frozen-clock reruns are byte-stable for packet + HTML report.
#
# The local fixture HTTP server always binds an OS-assigned ephemeral port
# (port 0) so two concurrent `npm run verify` runs never collide on a fixed
# port (see AL-129). To keep the byte-stability check below meaningful, the
# second of the two runs pins ALLIE_FIXTURE_PORT to whatever port the OS
# actually handed the first run, so both legs of this script observe the
# exact same port and every port-derived byte (base_url, state urls, axe
# JSON, report HTML) matches without needing to normalize anything.
set -eu

RUN_DIR=.allie/runs/v0-smoke
FREEZE_EPOCH=1700000000
FREEZE_RFC3339="2023-11-14T22:13:20+00:00"
FIRST="$(mktemp -d)"
cleanup() {
  rm -rf "$FIRST"
}
trap cleanup EXIT

run_fixture() {
  # No rm of $RUN_DIR here: the second invocation deliberately reruns into
  # the dirty --out so allie's own manifest cleanup (AL-117) is what removes
  # stale content. Pass a port to pin ALLIE_FIXTURE_PORT (byte-stability
  # leg); omit it for an OS-assigned ephemeral bind (AL-129).
  port="${1:-}"
  if [ -n "$port" ]; then
    ALLIE_FIXTURE_PORT="$port" \
    SOURCE_DATE_EPOCH="$FREEZE_EPOCH" \
      cargo run --locked -- run --manifest examples/login-flow.yml --out "$RUN_DIR"
  else
    SOURCE_DATE_EPOCH="$FREEZE_EPOCH" \
      cargo run --locked -- run --manifest examples/login-flow.yml --out "$RUN_DIR"
  fi
}

extract_fixture_port() {
  node - "$1" <<'NODE'
import fs from 'node:fs';

const [packetPath] = process.argv.slice(2);
const packet = JSON.parse(fs.readFileSync(packetPath, 'utf8'));
const match = /^http:\/\/127\.0\.0\.1:(\d+)\/$/.exec(packet.target.base_url);
if (!match) {
  throw new Error(`could not extract fixture port from base_url ${packet.target.base_url}`);
}
process.stdout.write(match[1]);
NODE
}

rm -rf "$RUN_DIR"
run_fixture
FREEZE_PORT="$(extract_fixture_port "$RUN_DIR/evidence.json")"
cp "$RUN_DIR/evidence.json" "$FIRST/evidence.json"
cp "$RUN_DIR/report.html" "$FIRST/report.html"

# AL-117 out-dir hygiene, run path, end to end: plant a stale artifact from a
# retired stage, then rerun into the SAME dirty --out with no rm -rf. Allie's
# own manifest-based cleanup must remove it, and the rerun must still be
# byte-stable against the fresh-directory first run (pinned to the same
# discovered port so every port-derived byte matches — AL-129).
mkdir -p "$RUN_DIR/remediation"
printf '%s\n' '{"stale": true}' > "$RUN_DIR/remediation/legacy-finding.json"

run_fixture "$FREEZE_PORT"
if [ -e "$RUN_DIR/remediation" ]; then
  echo "stale remediation sentinel survived a rerun into a dirty --out" >&2
  exit 1
fi
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
  && state.features?.mobile_viewport_checked === true
  && state.features?.mobile_viewport_width === 390
  && state.features?.mobile_viewport_height === 844
)), 'login-form state metadata must be contentful');
assert(packet.artifacts.some((artifact) => artifact.id === 'axe-json-login-form' && artifact.hash?.startsWith('sha256:')), 'axe artifact hash missing');
assert(packet.artifacts.some((artifact) => artifact.id === 'screenshot-login-form' && artifact.hash?.startsWith('sha256:')), 'screenshot artifact hash missing');
assert(packet.artifacts.some((artifact) => artifact.id === 'mobile-axe-json-login-form' && artifact.hash?.startsWith('sha256:')), 'mobile axe artifact hash missing');
assert(packet.artifacts.some((artifact) => artifact.id === 'mobile-screenshot-login-form' && artifact.hash?.startsWith('sha256:')), 'mobile screenshot artifact hash missing');

const deterministic = verdict('wcag22-aa:deterministic-axe-rules');
assert(deterministic?.status === 'pass', 'deterministic aggregate must pass');
assert(deterministic?.confidence === 'machine_proven', 'deterministic confidence must stay distinct');
const reflow = verdict('wcag22-aa:1.4.10-reflow');
assert(['pass', 'fail'].includes(reflow?.status), `reflow scripted verdict must be terminal, got ${reflow?.status}`);
assert(reflow?.confidence === 'script_observed', 'scripted confidence must stay distinct');
assert(packet.verdicts.some((item) => item.status === 'needs_review'), 'fixture must retain explicit review obligations');
assert(packet.verdicts.some((item) => item.status === 'not_applicable'), 'fixture must retain not_applicable obligations');

assert(report.includes('Allie evidence status'), 'report missing evidence status');
assert(report.includes('No deterministic axe failures'), 'report missing deterministic pass summary');
assert(report.includes('mobile viewport'), 'report missing mobile viewport evidence caption');
assert(report.includes('<h2>Replay</h2>'), 'report missing replay section');
NODE

test -f "$RUN_DIR/evidence.json"
test -f "$RUN_DIR/report.html"
test -f "$RUN_DIR/artifacts/axe-login-form.json"
test -f "$RUN_DIR/artifacts/login-form.png"
test -f "$RUN_DIR/artifacts/axe-mobile-login-form.json"
test -f "$RUN_DIR/artifacts/mobile-login-form.png"
echo "evidence smoke passed: byte-stable frozen packet/report with content assertions"
