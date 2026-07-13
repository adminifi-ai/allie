# Cross-Target Dogfood Summary: Vanity vs. Olympus

Date: 2026-07-01
Ticket: 025 (child 8 — cross-target summary that converts dogfood misses into
epics 026 and 027)

This compares the two dogfood slices delivered so far — the **Vanity** static/
public slice (`docs/dogfood/vanity/025-vanity-static-dogfood.md`) and the
**Olympus** authenticated operations/control-plane slice
(`docs/dogfood/olympus/025-olympus-dashboard-dogfood.md`) — and files the repeat
misses against epic 026 (autonomous mapping + test-generation eval lab) and epic
027 (WCAG report efficacy and quality). The point of the ladder is not a pass/fail
metric per target; it is whether Allie can explain what it saw across different
app classes with comparable, replayable receipts. It can — and the comparison
surfaces exactly where the report and mapping still fall short.

## Receipt matrix

| Dimension | Vanity (static/public) | Olympus (auth control-plane) |
|---|---|---|
| Target class | Public single-page static site | Authenticated Hono dashboard |
| Target commit | `68a15d0` (clean) | `a743f88` (clean) |
| Git status before == after | yes (untouched) | yes (untouched) |
| Auth | none | cookie password via `DASHBOARD_SECRET` (env name only) |
| Start harness | `python3 -m http.server` | bespoke `olympus-dashboard-harness.mjs` shim |
| Model / agentic pass | **enabled** (`gemini-3.5-flash`) | **disabled** (`zdr_required`) |
| Surfaces / states captured | 1 / 1 | 6 / 6 |
| Infrastructure failures | 0 | 0 (auth verify); 1 (negative control, by design) |
| Deterministic failures | 2 | 23 (+2 on the login smoke) |
| State / console / network errors | 0 / 1 (404 noise) / 0 | 0 / 0 / 0 |
| WCAG matrix (55 success criteria) | pass 39 · fail 1 · needs_review 2 · n/a 13 · not_tested 0 | pass 0 · fail 3 · needs_review 44 · not_tested 8 |
| Release status | blocked (deterministic) | blocked (deterministic) |
| Exit code | 1 | 1 |

## What held (invariants proven across both classes)

- **Never mutates the target.** Both target repos had byte-identical `git status`
  before and after; Vanity was checked twice.
- **Infra failures are separated from findings.** Both releases blocked purely on
  deterministic target evidence, with `infrastructure_failures: 0` on the audit
  runs. Olympus's negative-auth control correctly produced an infra `error` (exit
  2) instead of auditing the login wall as the app — no silent gap.
- **Deterministic gate is stable; model layer stays advisory.** Vanity was run
  twice: the deterministic spine (2 failures, `1.4.3`, release blocked) was
  byte-identical while agentic verdicts drifted (`needs_review` 4↔2). The gate
  rides only the stable spine.
- **Host-agnostic contract.** The same `allie verify` command drove both a static
  site and an authenticated dashboard.

## Findings → epic 026 (eval lab)

**E1 — Fixture-dir discovery over-generates non-served surfaces (mapping
precision).** The 2026-06-20 consumer-contract run on Vanity
(`docs/dogfood/014-vanity-consumer-contract.md`) synthesized **15** surfaces and
**32** deterministic failures from the filesystem; the pinned-manifest run on the
same site produces **1** real served surface and **2** failures. Fixture-dir
mapping invents surfaces with no served route. 026 needs a precision grader that
penalizes surfaces that do not resolve to a real route.
→ Reinforces 026 children 4 (surface-map recall grader) and 6 (mapping-miss
diagnostics distinct from app defects).

**E2 — Discovery recall is unmeasured on the manifest-pinned path.** Both slices
hand-specify their states (1 for Vanity, 6 for Olympus), so nothing measures
whether Allie *would have found* the right surfaces autonomously. Olympus reached
6 dashboard routes only because the manifest listed them.
→ Reinforces 026 children 1–4: a labeled corpus with expected surfaces is the
only way to score recall rather than assuming the manifest is ground truth.

**E3 — Authenticated targets impose harness/integration friction the eval should
score.** Olympus required a bespoke `olympus-dashboard-harness.mjs` because it has
no stable dashboard-only export and full boot verifies Sprite lanes; Vanity needed
none. The negative-auth control worked (correctly errored), but the setup cost is
real and uneven across the ladder.
→ Add a target-integration-friction / false-auth-wall-capture dimension to 026's
metric set (already named in the 026 oracle); track it per fixture class.

## Findings → epic 027 (report efficacy)

**R1 (HIGH) — Without the model, the report degrades into a wall of
needs_review/not_tested.** This is the single clearest cross-target result.
Model-enabled Vanity: pass 39 / needs_review 2 / not_tested 0. Model-disabled
Olympus: pass **0** / needs_review **44** / not_tested **8**. The exact failure
mode 027 names ("the report does not degrade into a wall of `needs_review`") is
reproduced whenever the target class forbids the model (ZDR/control-plane).
Judgment-heavy criteria need deterministic heuristics, grouping, and calibration
that work with the model **off**, or the report is only useful for public sites.
→ 027 child 4 (grouping/calibration for judgment-heavy criteria), scoped to the
model-off path explicitly.

**R2 (HIGH) — Headline `not_tested` can hide obligation-grain gaps (silent-gap
risk).** On Vanity the 55-success-criterion headline shows `not_tested: 0`, but
the finer 60-obligation coverage ledger lists **4** untested obligations
(`1.4.10-reflow`, `1.4.10-zoom-reflow`, `2.1.1-keyboard-traversal`,
`2.2.2-reduced-motion`) that roll up into passing criteria. A reader trusting the
headline would not know reflow/zoom/reduced-motion were not exercised. This rubs
directly against the VISION "no silent gaps" invariant.
→ 027 children 1 and 4: reconcile obligation vs success-criterion grain so
"not tested" stays visible after roll-up.

**R3 — The report exposes at least three unreconciled "review" counts.** For the
same Vanity run: `needs_review` 2 (compliance summary), review-needed obligations
**7** (release summary — printed in the *same* `allie-report.md` that says
"needs review 2"), and obligations-requiring-human-review **46** (evidence
coverage). They measure different things but the report never reconciles them, so
the number a consumer trusts depends on which artifact they open.
→ 027 child 4: one reconciled "here is what still needs review and why" view.

**R4 — Headline counts are not a safe cross-run diff signal.** Two identical
Vanity runs reported pass 37 vs 39 and needs_review 4 vs 2 purely from agentic
drift, while the deterministic gate was unchanged. A naive diff would report a
regression/improvement that did not happen.
→ 027 child 3 (evidence/report diff): classify changes as deterministic vs
agentic-advisory so drift is not read as a real change.

**R5 — `allie verify` does not clean or namespace its `--out` directory
(provenance/trust).** The first run of this session inherited stale artifacts from
a 2026-06-19 run — including a whole obsolete `remediation/` stage (`patch-plan.md`,
`remediation-queue.json`) that the current tool has since removed and that its
tests now forbid. A reader inspecting the packet dir would see fix suggestions
Allie no longer emits, appearing to contradict the no-remediation invariant. The
current tool is correct; the packet directory is not self-describing.
→ 027: emit a per-run file manifest (or clean/version the out-dir) so the packet
directory describes exactly one run. Small, high-trust win.

**R6 (minor) — Manifests under-declare known nondeterminism.** Vanity's manifest
sets `known_nondeterminism: []` yet the agentic layer demonstrably varies. The
gate is unaffected, but the declaration is inaccurate.
→ Fold into 027's report-quality checks or 026 fixture labeling.

## Proposed backlog actions

- Habitat epics 026 and 027 were updated in place with a "Dogfood findings
  (2026-07-01)" note pointing here.
- No new standalone ticket is warranted yet: E1–E3 land inside 026's existing
  children; R1–R6 land inside 027's. Revisit if R1 (model-off report degradation)
  proves large enough to be its own slice.
- Ladder status: static/public (Vanity ✓) and operations/control-plane (Olympus ✓)
  classes are covered. Remaining 025 classes: authenticated consumer (Linejam,
  refresh) and work-management (Habitat).

This is evidence visibility for accessibility engineering review, not a legal
compliance claim.
