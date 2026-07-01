# Make Allie agent-ready for QA and dogfood

Priority: P2 · Status: pending · Estimate: L

## Goal
Create repo-local agent instructions and runbooks that let a cold agent run Allie QA, dogfood, receipt capture, and report review without rediscovering commands or mutating target repositories.

## Oracle
- [ ] A repo-local QA/dogfood skill or equivalent runbook names canonical commands, target startup paths, receipt format, safe auth handling, and no-mutation boundaries.
- [ ] The runbook includes checklists for `allie verify`, report inspection, artifact inventory, target repo status capture, and residual-risk wording.
- [ ] The dogfood receipt template is reused by at least one target refresh.
- [ ] The repo explains how to distinguish target accessibility findings from Allie infra/auth/model failures.
- [ ] The instructions are referenced from `AGENTS.md`, `README.md`, or a docs index where cold agents will actually find them.

## Verification System
- Claim: A new agent can perform Allie QA/dogfood with the repo's real commands and produce a useful receipt on the first pass.
- Falsifier: The instructions omit a required command, encourage editing target repos, leak secrets/session state, conflate blocked auth with app findings, or diverge from `scripts/verify.sh`.
- Driver: Run the runbook against one safe fixture and one safe dogfood target.
- Grader: Receipt completeness checklist, before/after git status, artifact existence and semantic spot checks, and a short cold-read review.
- Evidence packet: `docs/dogfood/<target>/` receipt plus the generated `.allie/` artifacts.
- Cadence: Update when the gate, manifest, dogfood ladder, or report contract changes.

## Children
1. Add the repo-local QA/dogfood skill or runbook with the real Allie command sequence.
2. Add a dogfood receipt template with required fields and explicit residual-risk language.
3. Add target safety rules: record git status, do not edit target repos, keep storageState local/ignored, never print secrets.
4. Add troubleshooting sections for auth-lost, worker infra failure, model degradation, target deterministic findings, and missing artifacts.
5. Link the runbook from the repo's agent-facing docs.
6. Prove it by refreshing the safe Vanity or Olympus dogfood receipt from the runbook.

## Notes
- There is currently a strong root `AGENTS.md`, but no repo-local Allie QA skill/runbook was found in the checkout.
- This epic should reduce future operator supervision, not create a second workflow engine.
