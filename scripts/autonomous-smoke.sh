#!/bin/sh
set -eu

DISCOVERY_DIR=.allie/discovery/autonomous-smoke
RUN_DIR=.allie/runs/autonomous-smoke
REVIEW_DIR=.allie/reviews/autonomous-smoke
REMEDIATION_DIR=.allie/remediation/autonomous-smoke
RELEASE_DIR=.allie/releases/autonomous-smoke

rm -rf "$DISCOVERY_DIR" "$RUN_DIR" "$REVIEW_DIR" "$REMEDIATION_DIR" "$RELEASE_DIR"

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

cargo run --locked -- remediate \
  --packet "$RUN_DIR/evidence.json" \
  --out "$REMEDIATION_DIR"

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
node -e "const fs=require('fs'); const q=JSON.parse(fs.readFileSync('$REMEDIATION_DIR/remediation-queue.json','utf8')); if(q.schema!=='allie.remediation-queue.v0') process.exit(1); if(!q.items.length) process.exit(1);"
grep -q "Replay:" "$REMEDIATION_DIR/patch-plan.md"
node -e "const fs=require('fs'); const r=JSON.parse(fs.readFileSync('$RELEASE_DIR/release-summary.json','utf8')); if(r.status!=='blocked') process.exit(1);"

echo "autonomous smoke passed"
