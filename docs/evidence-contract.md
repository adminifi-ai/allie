# Evidence Contract

This is the first draft of the Allie evidence packet. The formal V0 JSON Schema lives at `schemas/allie.evidence.v0.schema.json`.

## Packet

```json
{
  "schema": "allie.evidence.v0",
  "summary": {},
  "run": {},
  "target": {},
  "policy": {},
  "coverage": {},
  "artifacts": [],
  "findings": [],
  "verdicts": [],
  "waivers": [],
  "review": [],
  "replay": {}
}
```

## Run Metadata

- `run.id`
- `run.started_at`
- `run.finished_at`
- `run.allie_version`
- `run.git_sha`
- `run.git_branch`
- `run.ci_provider`
- `run.actor`

## Target

- `target.base_url`
- `target.environment`
- `target.app_name`
- `target.auth_profile`
- `target.flow_manifest`

## Policy

- `policy.profile`: `wcag21-aa`, `wcag22-aa`, `section508`, `ada-title-ii`, or client pack.
- `policy.blocking_classes`
- `policy.model_provider_allowlist`
- `policy.zdr_required`
- `policy.redaction_profile`
- `policy.budget`

## Coverage

- routes visited;
- surfaces discovered;
- flows exercised;
- states captured;
- standards obligations evaluated;
- obligations not tested;
- obligations requiring human review.

## Artifacts

Artifact types:

- `screenshot`
- `video`
- `gif`
- `dom_snapshot`
- `accessibility_tree`
- `axe_json`
- `playwright_trace`
- `console_summary`
- `network_summary`
- `model_prompt`
- `model_response`
- `html_report`

Each artifact should carry:

- id;
- type;
- path or URI;
- hash;
- redaction status;
- related flow/state;
- creation tool;
- timestamp.

## Findings

Each finding should include:

- id;
- title;
- description;
- evidence class;
- standard obligation;
- severity;
- status;
- confidence;
- source;
- affected route/state;
- artifact refs;
- suggested remediation;
- replay command.

## Verdicts

Verdicts are obligation-level conclusions. They should be separate from raw findings.

Statuses:

- `pass`
- `fail`
- `not_applicable`
- `needs_review`
- `not_tested`
- `waived`
- `risk_accepted`

Confidence:

- `machine_proven`
- `script_observed`
- `agent_inferred`
- `human_attested`

## Replay

Every packet needs enough data to rerun the same path:

- command;
- manifest path;
- environment requirements;
- credential profile name, not secret value;
- browser settings;
- seed data requirements;
- known nondeterminism.

## Waivers

Waivers are packet-attached release inputs, not global dashboards. A waiver that
touches a changed surface must carry:

- `id`;
- `surface`;
- `status`: `waived` or `risk_accepted`;
- `provenance`: actor/reason/review source;
- `expires_at`;
- `packet_ref` or `packet_refs`.

Release projection blocks expired touched waivers and touched waivers missing
the required metadata.

## Release Decision Projection

`allie release --packet <evidence.json> --out <dir> --changed-surface <id>`
projects a packet into:

- `release-summary.json`;
- `github-check.json`;
- `release-report.html`.

The projection blocks on deterministic/scripted/infrastructure packet failures,
missing required evidence for changed surfaces, expired touched waivers, and
invalid touched-waiver metadata. It marks stale evidence, model-only findings,
`needs_review`, and `not_tested` obligations as review-required neutral outputs
instead of hard release blocks.
