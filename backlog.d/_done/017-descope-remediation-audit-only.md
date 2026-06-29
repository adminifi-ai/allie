# Descope remediation — Allie audits, maps, and reports only

Priority: P1 · Status: done · Estimate: M

## Goal
Remove remediation from Allie's scope. Allie audits, maps, and reports; fixing
what it finds is a separate downstream product that consumes Allie's evidence.

## Oracle
- [x] `allie remediate` CLI removed.
- [x] Remediation step removed from the workbench loop and job pointer.
- [x] `suggested_remediation` simplified, not re-engineered: fresh evidence no
  longer emits it, and the legacy schema field is optional rather than
  required.
- [x] SPEC.md "Remediation Model" deleted; README/docs/AGENTS reframed
  audit-only; VISION.md aligned.
- [x] `autonomous:smoke` and `verify.sh` now assert remediation artifacts are
  absent; all gates green.

## Children
1. Remove `allie remediate` CLI (dispatch, parser, structs, usage line, tests).
2. Remove remediation step from the workbench loop + job pointer.
3. Delete `run_remediate`/queue/ledger/patch-plan/report renderers.
4. Simplify `suggested_remediation`: relax/drop the `required` schema constraint; let diagnostic context live as natural report prose (version bump for the schema relaxation).
5. Strip remediation from `autonomous-smoke.sh` + `verify.sh` asserts.
6. Delete SPEC "Remediation Model"; reframe README/docs/AGENTS to audit-only; note ticket 010 superseded.
7. Purge generated `.allie/**/remediation` artifacts.

## Notes
**Why:** remediation-descope lane (full inventory, all locations vetted) enacting the vision delta. **`suggested_remediation` — decided (keep it simple):** the downstream remediation product is an LLM agent that reads fuzzy, unstructured text, so it needs the *information*, not a rigid required field. Drop the `required` constraint and let diagnostic context (what's wrong, where, evidence links) live as natural report prose; do not engineer a replacement structured field. Over-constraining a model-facing interface is cost without payoff (the bitter lesson). **Breaking change:** removing the `remediate` subcommand + relaxing the schema is a public CLI/packet-version bump. **Safe:** `allie verify` never calls remediate (consumer.rs:324-375). The `required_for_remediation_signoff` status (compliance.rs:580) drives a real fail gate — rename only, keep the logic. Delete-before-add: shrinks lib.rs ahead of epic 019.

## Delivered

- Removed the public `allie remediate` command, remediation queue/action ledger
  rendering, patch-plan output, and workbench remediation step.
- Reframed Allie as audit/map/report/release projection only across the vision,
  spec, README, verification docs, architecture notes, roadmap, and consumer
  docs.
- Kept downstream fixing out of Allie's product surface while preserving the
  evidence packet and release projection as the handoff contract.
- Added a local HTML execution plan at
  `docs/plans/017-remediation-descope.html`.

## Verification

- `cargo test --locked remediation_cli_is_not_part_of_allie -- --nocapture`
- `cargo check --locked`
- `cargo test --locked --lib`
- `npm run autonomous:smoke`
- `npm run verify`
- Live CLI probe: `cargo run --locked -- remediate` exits 64 and reports
  `allie: unknown command`.
- Artifact probe: autonomous workbench output has no remediation pointer, no
  remediation step, and no `.allie/remediation/autonomous-smoke` directory.
- Fresh-context review: initial blocker found stale legacy remediation cleanup;
  fixed it with legacy cleanup plus negative smoke/verify assertions.
- Fresh-context re-review: `BLOCKING: no`; findings: none.
