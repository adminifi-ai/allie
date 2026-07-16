# Evidence Contract

This is the first draft of the Allie evidence packet. The formal V0 JSON Schema lives at `schemas/allie.evidence.v0.schema.json`.

`allie.evidence.v0` is not guaranteed to deserialize across Allie version bumps — fields have been renamed as their semantics were clarified (e.g. `coverage.obligations_requiring_human_review` → `coverage.profile_human_review_scope`), with no `serde` back-compat aliasing; re-run `allie run`/`allie verify` against the current binary rather than persisting `evidence.json` artifacts across versions.

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

`run.git_sha` and `run.git_branch` are required provenance, not best-effort
metadata. `allie run` records them from the Git checkout that contains the
manifest path, or from `--project-root <dir>` when the caller supplies an
explicit project root. A run against a directory with no Git commit fails as an
infrastructure/provenance error instead of writing empty revision fields.

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
- `policy.model_egress_redaction`: `none` for an enabled V0 model route, or
  `null` when model review is disabled. This is the accepted egress mode, not a
  claim that media was transformed.
- `policy.redaction_profile`
- `policy.budget`

`policy.redaction_profile` describes retained local artifacts. It is separate
from model egress. Every agentic worker response carries
`allie.model-redaction-receipt.v0`: profile `none` is `not_sent` when no model
call occurred and `not_applied` after unredacted media was transmitted. V0 has
no status that can honestly serialize `applied` or `redacted`.

Each accepted gateway run also appends an `allie.model-egress-event.v0` entry
under `model_egress_events`. The event binds the opaque prompt version,
provider/model/endpoint, call and token counts, redaction receipt, outcome, and
SHA-256 hashes of the exact worker request and response. It contains no API key
or prompt/media body. A worker response with an absent or unrecognized prompt
version is rejected before its assessments or audit event enter the packet.

## Coverage

- routes visited;
- surfaces discovered;
- flows exercised;
- states captured;
- standards obligations evaluated;
- obligations not tested;
- obligations requiring human review.

Manifest states may declare non-secret `steps` (`fill`, `type`, `click`, and
`wait_for`) that the browser worker performs after navigation and before
capturing evidence. The resulting artifacts represent the post-action state.
Step failures are recorded as state errors so required evidence blocks instead
of silently passing. Credential values do not belong in state steps; use
`auth.steps` with env-var names for secret-bearing login recipes.

## Discovery

`allie discover --manifest <flow.yml> --out <dir>` writes a discovery packet and
flow plan. For `target.kind: local_fixture`, discovery reads checked-in HTML
fixtures. For live web targets, discovery starts at an `http://`
`target.base_url`, fetches same-origin HTML pages, extracts links, and records
bounded route candidates from links and same-origin `/sitemap.xml` entries.
External hosts, fragments, asset links, and non-HTTP schemes are ignored. HTTPS,
credentialed crawling, and JavaScript-driven navigation discovery are follow-on
layers, not implied by this packet.

Discovery output is a candidate map, not a legal or complete-coverage claim.
Manifest-declared states remain authoritative and are merged with discovered
routes; generated flows must still replay through `allie run` before release
enforcement. Live crawl failures are recorded in the packet's `diagnostics`
array and the product map's `discovery_diagnostics` array instead of being
silently converted into clean manifest-only coverage.

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
- publication class (`sensitive_local`, `redacted_shareable`, or
  `public_summary`);
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
- `profile_views`: derived views over the same ledger, including the
  `wcag21-aa` EAA-oriented projection with WCAG 2.2-only criteria excluded and
  WCAG 2.1-only legacy gaps called out explicitly;
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

Captured web states include mobile-web viewport metadata when the worker can
run it:

- `coverage.state_metadata[].features.mobile_viewport_checked`;
- `coverage.state_metadata[].features.mobile_viewport_width`;
- `coverage.state_metadata[].features.mobile_viewport_height`.

The worker writes mobile screenshot and mobile axe artifacts beside the primary
state artifacts. Mobile-relevant WCAG criteria stay in the criterion matrix at
mobile viewport evidence depth; criteria that still need visual, pointer,
motion, or orientation judgment remain `needs_review` instead of being counted
as pass.

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

GitHub, Azure, and local runs use the same verify command and reporter names.
The canonical `.allie/verify/latest` tree is sensitive local evidence. Public
host wrappers run `allie publication --verify-root .allie/verify/latest --out
.allie/public/latest` and publish only its four explicitly allowlisted files. Private
evidence stores may retain the canonical tree under their own access policy;
host wrappers do not define accessibility policy. The projection and its
receipt are `public_summary`; a refusal classifies the requested artifact as
`sensitive_local` without copying or echoing its raw path.

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
