# Descope remediation — Allie audits, maps, and reports only

Priority: P1 · Status: pending · Estimate: M

## Goal
Remove remediation from Allie's scope. Allie audits, maps, and reports; fixing
what it finds is a separate downstream product that consumes Allie's evidence.

## Oracle
- [ ] `allie remediate` CLI removed (dispatch src/lib.rs:505, parser:848, run_remediate:2157, report:2230, patch-plan:2250, structs:152/228/3259, usage:564, tests:5905).
- [ ] Remediation step removed from the workbench loop + job pointer (workbench.rs:215,417-445,428).
- [ ] `suggested_remediation` simplified, not re-engineered: drop the rigid `required` schema constraint; keep any genuinely diagnostic context (what's wrong, where, evidence links) in the report's natural prose for a downstream LLM agent to read. No new structured field.
- [ ] SPEC.md "Remediation Model" deleted; README/docs/AGENTS reframed audit-only; VISION.md aligned.
- [ ] `autonomous:smoke` + `verify.sh` remediation asserts removed; all gates green.

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
