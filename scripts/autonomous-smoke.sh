#!/bin/sh
set -eu

DISCOVERY_DIR=.allie/discovery/autonomous-smoke
RUN_DIR=.allie/runs/autonomous-smoke
REVIEW_DIR=.allie/reviews/autonomous-smoke
RELEASE_DIR=.allie/releases/autonomous-smoke
JOB_DIR=.allie/jobs/autonomous-smoke
LEGACY_REMEDIATION_DIR=.allie/remediation/autonomous-smoke

rm -rf "$DISCOVERY_DIR" "$RUN_DIR" "$REVIEW_DIR" "$RELEASE_DIR" "$JOB_DIR" "$LEGACY_REMEDIATION_DIR"

cargo run --locked -- discover \
  --manifest examples/autonomous-workbench.yml \
  --out "$DISCOVERY_DIR"

cargo run --locked -- promote-flow \
  --discovery "$DISCOVERY_DIR/discovery.json" \
  --flow-plan "$DISCOVERY_DIR/flow-plan.json" \
  --out "$DISCOVERY_DIR/generated-flow.yml"

set +e
cargo run --locked -- run \
  --manifest "$DISCOVERY_DIR/generated-flow.yml" \
  --out "$RUN_DIR"
run_status=$?
set -e
test "$run_status" -eq 1

cargo run --locked -- review \
  --packet "$RUN_DIR/evidence.json" \
  --out "$REVIEW_DIR"

set +e
cargo run --locked -- release \
  --packet "$REVIEW_DIR/evidence-reviewed.json" \
  --out "$RELEASE_DIR" \
  --changed-surface settings
release_status=$?
set -e
test "$release_status" -eq 1

node -e "const fs=require('fs'); const p=JSON.parse(fs.readFileSync('$DISCOVERY_DIR/discovery.json','utf8')); if(p.schema!=='allie.discovery.v0') process.exit(1); if(!p.surfaces.some(s=>s.id==='settings')) process.exit(1);"
node -e "const fs=require('fs'); const p=JSON.parse(fs.readFileSync('$RUN_DIR/evidence.json','utf8')); const types=new Set(p.artifacts.map(a=>a.type)); for (const t of ['axe_json','screenshot','dom_snapshot','accessibility_tree','trace','html_report']) if(!types.has(t)) process.exit(1); if(!p.verdicts.some(v=>v.obligation==='wcag22-aa:2.4.11-focus-not-obscured-minimum')) process.exit(1);"
node -e "const fs=require('fs'); const p=JSON.parse(fs.readFileSync('$REVIEW_DIR/evidence-reviewed.json','utf8')); if(!p.review.length) process.exit(1); if(!p.findings.some(f=>f.evidence_class==='agentic')) process.exit(1);"
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
test -f "$JOB_DIR/steps/review/evidence-reviewed.json"
test ! -d "$JOB_DIR/steps/remediation"
test ! -d "$LEGACY_REMEDIATION_DIR"
test -f "$JOB_DIR/steps/release/release-summary.json"

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
