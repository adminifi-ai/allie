#!/bin/sh
set -eu

DISCOVERY_DIR=.allie/discovery/autonomous-smoke
RUN_DIR=.allie/runs/autonomous-smoke
RELEASE_DIR=.allie/releases/autonomous-smoke
JOB_DIR=.allie/jobs/autonomous-smoke
AGENTIC_JOB_DIR=.allie/jobs/autonomous-agentic-smoke
AGENTIC_ERROR_JOB_DIR=.allie/jobs/autonomous-agentic-error-smoke
AGENTIC_ERROR_WORKER=.allie/jobs/autonomous-agentic-error-worker.cjs
LEGACY_REMEDIATION_DIR=.allie/remediation/autonomous-smoke

rm -rf "$DISCOVERY_DIR" "$RUN_DIR" "$RELEASE_DIR" "$JOB_DIR" "$AGENTIC_JOB_DIR" "$AGENTIC_ERROR_JOB_DIR" "$LEGACY_REMEDIATION_DIR"
rm -f "$AGENTIC_ERROR_WORKER"

cargo run --locked -- discover \
  --manifest examples/autonomous-workbench.yml \
  --out "$DISCOVERY_DIR"

cargo run --locked -- promote-flow \
  --discovery "$DISCOVERY_DIR/discovery.json" \
  --flow-plan "$DISCOVERY_DIR/flow-plan.json" \
  --out "$DISCOVERY_DIR/generated-flow.yml"

node - "$DISCOVERY_DIR/flow-plan.json" <<'NODE'
const fs = require('fs');
const flowPlan = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const byId = new Map(flowPlan.candidates.map((candidate) => [candidate.id, candidate]));
const home = byId.get('home');
const settings = byId.get('settings');
if (!home?.steps?.some((step) => step.click?.selector === '#open-menu')) {
  throw new Error('generated home candidate did not click the actions menu');
}
if (!home.steps.some((step) => step.wait_for?.selector === '#menu:not([hidden])')) {
  throw new Error('generated home candidate did not wait for the revealed menu');
}
if (!settings?.steps?.some((step) => step.fill?.selector === '#email')) {
  throw new Error('generated settings candidate did not fill the email field');
}
if (!settings.steps.some((step) => step.type?.text === '.typed')) {
  throw new Error('generated settings candidate did not type into the email field');
}
if (!settings.steps.some((step) => step.wait_for?.selector === '#email-preview[data-ready]')) {
  throw new Error('generated settings candidate did not wait for the email preview readiness signal');
}
NODE
grep -q "steps:" "$DISCOVERY_DIR/generated-flow.yml"
grep -q "#open-menu" "$DISCOVERY_DIR/generated-flow.yml"
grep -q "qa@example.test" "$DISCOVERY_DIR/generated-flow.yml"

set +e
cargo run --locked -- run \
  --manifest "$DISCOVERY_DIR/generated-flow.yml" \
  --out "$RUN_DIR"
run_status=$?
set -e
test "$run_status" -eq 1

set +e
cargo run --locked -- release \
  --packet "$RUN_DIR/evidence.json" \
  --out "$RELEASE_DIR" \
  --changed-surface settings
release_status=$?
set -e
test "$release_status" -eq 1

node -e "const fs=require('fs'); const p=JSON.parse(fs.readFileSync('$DISCOVERY_DIR/discovery.json','utf8')); if(p.schema!=='allie.discovery.v0') process.exit(1); if(!p.surfaces.some(s=>s.id==='settings')) process.exit(1);"
node -e "const fs=require('fs'); const p=JSON.parse(fs.readFileSync('$RUN_DIR/evidence.json','utf8')); const types=new Set(p.artifacts.map(a=>a.type)); for (const t of ['axe_json','screenshot','dom_snapshot','accessibility_tree','trace','html_report']) if(!types.has(t)) process.exit(1); if(!p.verdicts.some(v=>v.obligation==='wcag22-aa:2.4.11-focus-not-obscured-minimum')) process.exit(1);"
node - "$RUN_DIR/evidence.json" "$RUN_DIR/artifacts/dom-home.html" "$RUN_DIR/artifacts/dom-settings.html" <<'NODE'
const fs = require('fs');
const packet = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
for (const id of ['home', 'settings']) {
  const state = packet.coverage.state_metadata.find((item) => item.id === id);
  if (!state) throw new Error(`missing generated state metadata for ${id}`);
  if (state.state_errors.length !== 0) {
    throw new Error(`generated state ${id} had state errors: ${JSON.stringify(state.state_errors)}`);
  }
}
const homeDom = fs.readFileSync(process.argv[3], 'utf8');
if (!homeDom.includes('id="menu"') || !homeDom.includes('Manage account settings')) {
  throw new Error('generated home DOM did not capture the revealed actions menu');
}
if (homeDom.includes('id="menu" hidden')) {
  throw new Error('generated home DOM captured the actions menu while still hidden');
}
const settingsDom = fs.readFileSync(process.argv[4], 'utf8');
if (!settingsDom.includes('qa@example.test.typed')) {
  throw new Error('generated settings DOM did not capture typed email evidence');
}
if (!settingsDom.includes('id="email-preview"') || !settingsDom.includes('data-ready')) {
  throw new Error('generated settings DOM did not capture the ready email preview');
}
NODE
node -e "const fs=require('fs'); const p=JSON.parse(fs.readFileSync('$RUN_DIR/evidence.json','utf8')); if('review' in p) process.exit(1); if(p.findings.some(f=>f.evidence_class==='agentic')) process.exit(1);"
node -e "const fs=require('fs'); const r=JSON.parse(fs.readFileSync('$RELEASE_DIR/release-summary.json','utf8')); if(r.status!=='blocked') process.exit(1);"

set +e
cargo run --locked -- workbench start \
  --manifest examples/autonomous-workbench.yml \
  --out "$JOB_DIR"
workbench_status=$?
set -e
test "$workbench_status" -eq 1

cargo run --locked -- workbench status \
  --job "$JOB_DIR"

node -e "const fs=require('fs'); const j=JSON.parse(fs.readFileSync('$JOB_DIR/job.json','utf8')); if(j.schema!=='allie.job.v0') process.exit(1); if(j.status!=='blocked') process.exit(1); if(j.runtime_policy.agent_step_timeout_ms!==null) process.exit(1); for (const p of ['product_map','compliance_report','evidence_packet','reviewed_packet','release_summary']) if(!j.pointers[p]) process.exit(1);"
node -e "const fs=require('fs'); const events=fs.readFileSync('$JOB_DIR/events.jsonl','utf8').trim().split('\\n').map(JSON.parse); if(!events.some(e=>e.event==='job_started')) process.exit(1); if(!events.some(e=>e.event==='step_completed' && e.step==='map')) process.exit(1); if(!events.some(e=>e.event==='job_finished')) process.exit(1);"
test -f "$JOB_DIR/steps/discovery/discovery.json"
test -f "$JOB_DIR/steps/map/product-map.json"
test -f "$JOB_DIR/steps/run/evidence.json"
test -f "$JOB_DIR/steps/report/compliance-report.json"
test ! -d "$JOB_DIR/steps/review"
test ! -d "$JOB_DIR/steps/remediation"
test ! -d "$LEGACY_REMEDIATION_DIR"
test -f "$JOB_DIR/steps/release/release-summary.json"
node -e "const fs=require('fs'); const p=JSON.parse(fs.readFileSync('$JOB_DIR/steps/run/evidence.json','utf8')); if('review' in p) process.exit(1); if(p.findings.some(f=>f.evidence_class==='agentic')) process.exit(1);"

set +e
env -u ALLIE_AGENTIC_WORKBENCH_SMOKE_KEY cargo run --locked -- workbench start \
  --manifest examples/autonomous-workbench-agentic.yml \
  --out "$AGENTIC_JOB_DIR"
agentic_workbench_status=$?
set -e
test "$agentic_workbench_status" -eq 1

node - "$AGENTIC_JOB_DIR" <<'NODE'
const fs = require('fs');
const path = require('path');
const jobDir = process.argv[2];
const job = JSON.parse(fs.readFileSync(path.join(jobDir, 'job.json'), 'utf8'));
if (job.status !== 'blocked') throw new Error(`expected blocked job, got ${job.status}`);
if (job.pointers.reviewed_packet !== 'steps/run/evidence.json') {
  throw new Error(`live agentic review should update the run packet, got ${job.pointers.reviewed_packet}`);
}
if (job.pointers.review_report) {
  throw new Error('live agentic review should not write the offline review report pointer');
}
const stepOrder = job.steps.map((step) => step.id).join('>');
if (!stepOrder.includes('run>review>report>release')) {
  throw new Error(`review must run before report/release, got ${stepOrder}`);
}
for (const rel of [
  'steps/run/agentic-request.json',
  'steps/run/agentic-response.json',
  'steps/run/evidence.json',
  'steps/report/compliance-report.json',
  'steps/release/release-summary.json',
]) {
  if (!fs.existsSync(path.join(jobDir, rel))) throw new Error(`missing ${rel}`);
}
if (fs.existsSync(path.join(jobDir, 'steps/review/evidence-reviewed.json'))) {
  throw new Error('model-enabled workbench used the offline review packet path');
}
const packet = JSON.parse(fs.readFileSync(path.join(jobDir, 'steps/run/evidence.json'), 'utf8'));
if (!packet.agentic_assessments.length) {
  throw new Error('live agentic gateway wrote no assessments');
}
if (!packet.agentic_assessments.some((assessment) => assessment.assessment === 'inconclusive' && assessment.media.length > 0)) {
  throw new Error('missing degraded live-gateway assessment with captured media');
}
const request = JSON.parse(fs.readFileSync(path.join(jobDir, 'steps/run/agentic-request.json'), 'utf8'));
const surfaceIds = new Set((request.surfaces || []).map((surface) => surface.id));
for (const id of ['home', 'settings']) {
  if (!surfaceIds.has(id)) throw new Error(`live agentic request missing ${id} review surface`);
}
const mediaCaptions = packet.agentic_assessments.flatMap((assessment) => assessment.media.map((entry) => entry.caption));
if (!mediaCaptions.some((caption) => caption.includes('settings'))) {
  throw new Error('live agentic assessment did not retain settings-surface media');
}
NODE

mkdir -p "$(dirname "$AGENTIC_ERROR_WORKER")"
cat > "$AGENTIC_ERROR_WORKER" <<'NODE'
const fs = require('fs');
const path = require('path');
const responseIndex = process.argv.indexOf('--response');
if (responseIndex === -1) process.exit(2);
const responsePath = process.argv[responseIndex + 1];
fs.mkdirSync(path.dirname(responsePath), { recursive: true });
fs.writeFileSync(responsePath, `${JSON.stringify({
  schema: 'allie.agentic.response.v0',
  status: 'error',
  errors: ['synthetic agentic worker failure'],
}, null, 2)}\n`);
process.exit(1);
NODE

set +e
ALLIE_AGENTIC_WORKER="$PWD/$AGENTIC_ERROR_WORKER" cargo run --locked -- workbench start \
  --manifest examples/autonomous-workbench-agentic.yml \
  --out "$AGENTIC_ERROR_JOB_DIR"
agentic_error_status=$?
set -e
test "$agentic_error_status" -eq 2

node - "$AGENTIC_ERROR_JOB_DIR" <<'NODE'
const fs = require('fs');
const path = require('path');
const jobDir = process.argv[2];
const job = JSON.parse(fs.readFileSync(path.join(jobDir, 'job.json'), 'utf8'));
if (job.status !== 'failed') throw new Error(`expected failed job, got ${job.status}`);
const review = job.steps.find((step) => step.id === 'review');
if (!review) throw new Error('missing review step');
if (review.status !== 'failed') throw new Error(`review step should fail, got ${review.status}`);
if (!review.message.includes('synthetic agentic worker failure')) {
  throw new Error(`review failure did not preserve worker error: ${review.message}`);
}
if (job.pointers.compliance_report || job.pointers.release_summary) {
  throw new Error('report/release should not run after agentic worker infrastructure failure');
}
const events = fs.readFileSync(path.join(jobDir, 'events.jsonl'), 'utf8').trim().split('\n').map(JSON.parse);
if (!events.some((event) => event.event === 'step_completed' && event.step === 'review' && event.status === 'failed')) {
  throw new Error('missing failed review event');
}
NODE

set +e
cargo run --locked -- workbench start \
  --manifest examples/autonomous-workbench.yml \
  --out "$JOB_DIR"
reuse_status=$?
cargo run --locked -- workbench start \
  --manifest examples/autonomous-workbench.yml \
  --out .allie/jobs/autonomous-smoke-opencode \
  --agent opencode
agent_status=$?
set -e
test "$reuse_status" -eq 2
test "$agent_status" -eq 64

echo "autonomous smoke passed"
