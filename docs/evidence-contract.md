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

## Compliance Report

`allie report --map <product-map.json> --packet <evidence.json> --out <dir>`
projects packet verdicts into a review surface. For `wcag22-aa`, report
summary counts use the 55 WCAG 2.2 A/AA success criteria only. Support checks
and aggregate gates remain visible in `supporting_checks`, but are excluded from
the WCAG denominator.

The report includes:

- `criteria`: one row per WCAG success criterion;
- `criterion_coverage`: one cell per criterion, surface, and captured state;
- `supporting_checks`: deterministic, scripted, agentic, and human aggregate
  checks linked to affected WCAG criteria;
- `surfaces`: product surface rollups with their cell refs and finding refs.

Each criterion coverage cell carries `status`, `applicability`, `method`,
`confidence`, `evidence_refs`, `agentic_refs`, `waiver_refs`, and
`residual_review_need`. Cells with `pass`, `fail`, `waived`, or
`risk_accepted` must carry provenance through evidence, agentic, waiver,
finding, artifact, or test refs; a replay command is drilldown context but is
not enough by itself. Model-only findings remain review context until scripted
or human-attested evidence promotes them.

## Verify Reporter Contract

`allie verify --manifest .allie/manifest.yml --out .allie/verify/latest`
is the host-agnostic consuming-app command. It runs discovery, generated-flow
promotion, product mapping, evidence replay, WCAG reporting, and release
projection, then writes a stable reporter matrix under `reporters/`:

- `allie-report.json`: `allie.verify.v0` summary with paths to all generated
  artifacts;
- `allie-compliance-report.json`: stable JSON copy of the WCAG report;
- `allie-report.html`: local drilldown entrypoint linking to map, report, and
  release artifacts;
- `allie-report.md`: Markdown summary for terminals and pull request bodies;
- `junit.xml`: one-suite CI reporter for pass/fail/error ingestion;
- `allie.sarif`: SARIF 2.1.0 summary for hosts that ingest code-scanning
  artifacts.

GitHub, Azure, and local runs use the same command and reporter names. Host
wrappers archive `.allie/verify/latest` as the artifact root so HTML drilldown
links can reach sibling map, evidence, report, and release outputs; they do not
define accessibility policy.

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
