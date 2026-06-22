# WCAG Criteria Assessability — Research Notes

Status: **Exploratory research, captured 2026-06-22. NOT committed design.**
This note captures a multi-lane research pass into making the judgment-heavy
("needs review") WCAG 2.2 AA criteria tractable for automated and agentic
assessment. Whether and how it informs the product is an open question pending
further research and design — treat everything here as input, not decision.

## Why this exists

Allie commits a pass/fail verdict per WCAG 2.2 AA success criterion. A minority
are settled deterministically (axe), a couple by scripted checks, and the
judgment-heavy remainder (~30 of the 44 `human_review`-method criteria) go to an
agentic vision review. Some still return "inconclusive" → `needs_review`. This
note maps how those could become higher-confidence assessments.

Industry baseline for "how much of WCAG is automatable" sits near **~30% of
success criteria**:

- Detlev Fischer (W3C WAI-GL, 2019): of 50 WCAG 2.1 A/AA SCs, only **3 fully
  automatable** (1.4.3, 3.1.1, 4.1.1); ~16 partly; rest manual.
- Deque: automated catches ~57% of issues *by volume* (contrast dominates),
  ~80% with guided manual; axe-core fully automates **~29.5% of WCAG 2.2 SCs**.
- Level Access: ~30% of SCs / ~40% of issues automatable; **~70% need human review**.

The goal of this research was to find the levers that push past that baseline.

## The reframe (the load-bearing idea)

Two independent research lanes converged on the same move:

> Stop asking a model "does this page pass SC X?" Instead **manufacture
> evidence** — drive a deterministic probe or a real oracle (screen reader,
> perturbation delta, constrained persona), capture its *observable output*, and
> have the model judge that artifact, not raw pixels. **Instrument the effect,
> don't eyeball it.** The delta / mismatch / announcement *is* the evidence.

Confidence then becomes meaningful: the degree of agreement between what is
**shown** (pixels), **structured** (the accessibility tree), and **spoken** (a
screen reader). The architecture that falls out is a three-tier evidence stack:

1. **Deterministic** — axe/ACT rules, DOM/geometry/CSSOM probes, a11y-tree
   assertions, Harding luminance analysis, caption diff, mutation deltas. Owns
   everything machine-decidable; emits hard pass/fail.
2. **Oracle-capture** — virtual + real screen reader, interaction probes,
   constrained-persona runs, video model. Produces observable artifacts.
3. **Model-jury** — judges only those artifacts; inter-rater agreement is the
   calibrated confidence; a distilled small judge handles the long tail.

## Capability buckets

The judgment criteria are not one problem — they fall into five buckets, each
needing a different lever.

### A. "Secretly deterministic" — a probe, no AI

| Criterion | Probe | Confidence | Prior art |
|---|---|---|---|
| 2.5.8 Target Size | bounding-box ≥24×24 + spacing-exception math | Deterministic | axe `target-size`; Alfa `sia-r113` |
| 2.5.3 Label in Name | accessible-name ⊇ visible text | Deterministic | axe `label-content-name-mismatch`; ACT `2ee8b8`; `dom-accessibility-api` |
| 2.4.11 Focus Not Obscured | focus rect vs `elementsFromPoint` z-stack | Deterministic (full occlusion) | IBM equal-access |
| 1.4.4 Resize Text | viewport-meta check + 200% reflow | Det. (static) + heuristic | ACT `b4f0c3`/`59br37`; axe `meta-viewport` |
| 1.3.5 Identify Input Purpose | valid `autocomplete` token | Det. (token) | axe `autocomplete-valid`; ACT `73f2c2` |
| 2.4.7 Focus Visible | computed-style diff on `:focus` | High heuristic | ACT `oj04fd` |
| 1.4.12 Text Spacing | inject WCAG spacing CSS → bbox clip/overlap | High heuristic | ACT `24afc2`/`78fd32`/`9e45ec`; TPGi bookmarklet |
| 1.3.4 Orientation | CSSOM scan for orientation lock | High heuristic | axe `css-orientation-lock`; ACT `b33eff` |

Notable: even Deque keeps 1.4.11, 2.4.7, 1.4.12 as *manual guided tests*, so
working probes here are a differentiator. **1.4.11 Non-text Contrast** stays
vision-routed (the "which pixels are necessary" problem is real; Evinced uses CV
here). Alfa (Siteimprove) is the only mainstream engine shipping `sia-r113`
(target size) and `sia-r115` (descriptive heading) — open-source, portable.

### B. Interaction-dependent — instrumentation harness, mostly deterministic

Inject one probe before navigation (`focusin/out`, pointer events, `popstate`,
`MutationObserver`, `activeElement`, URL); run a scripted action; diff before/after.

| Criterion | Action → instrumented verdict |
|---|---|
| 3.2.1 On Focus / 3.2.2 On Input | focus/change a control → context-change diff (deterministic, high) |
| 2.1.2 No Keyboard Trap | tab N≫count → focus cycles, never escapes (deterministic; ACT `80af7b` family) |
| 4.1.3 Status Messages | trigger update → was new text in a live region / focused? (deterministic verdict) |
| 2.5.2 Pointer Cancellation | `pointerdown` then up-elsewhere → fired on down? (mostly deterministic) |
| 1.4.13 Content on Hover/Focus | hover/focus → dismissible / hoverable / persistent (3 booleans) |
| 3.3.1/3.3.7/3.3.8 | submit invalid form / re-auth → error association, redundant-entry, paste-blocking |

Prior art: Apple **AXNav** (CHI'24 — agent acts, heuristic decides, video is
evidence), **GenA11y** (FSE'25 — 37 criteria, 94.5% precision), Skyvern
(form-fill). ARIA APG keyboard-interaction contracts + **ARIA-AT** are the
expected-behavior oracles to assert against. Reusable patterns from Browser-Use,
Stagehand (cache-the-action-map → replay free), and the WebVoyager/WebArena
observation space (a11y-tree + augmented screenshot + HTML together).

### C. Multi-page / site-level — cross-page comparison pass

The worker already captures DOM + full a11y tree + screenshot + focus order per
page; this is mostly a deterministic comparison pass + WCAG-EM sampling.

- **2.4.5 Multiple Ways** → count location mechanisms (nav/search/sitemap/breadcrumb) ≥2 — near-deterministic
- **3.2.3 Consistent Navigation** → nav fingerprint, Kendall-tau *order* distance (not set equality) + visual-vs-DOM order check
- **3.2.6 Consistent Help** → help-mechanism order consistency
- **3.2.4 Consistent Identification** → match "same-function" components (deterministic anchors: id/href/icon-hash → accessible-name embedding → screenshot-crop embedding → Hungarian assignment); the cosine margin rides into the confidence

Wildest: a per-template **site-baseline graph** so consistency checks become
git-trackable regressions (the PR that reorders a footer lights up as a 3.2.3
diff). Cost grows with *template count*, not page count.

### D. Media / temporal — mostly deterministic (do NOT use a VLM for seizures)

- **2.3.1 Three Flashes** → reproduce the **Harding Flash & Pattern Analyzer**
  algorithm (>3 flashes/s over ≥10% of field + red-flash threshold). Deterministic.
- **2.2.2 Pause/Stop/Hide, 2.2.1 Timing** → DOM/behavior (motion >5s + pause
  control; meta-refresh / `setInterval`). Deterministic.
- **1.4.2 Audio Control** → `autoplay` without `muted`. Deterministic.
- **Captions (1.2.x)** → `<track>` presence + **WhisperX** forced-alignment to
  check caption *accuracy*.
- Motion *description* → native-video models (e.g. Gemini 2.5 ingests mp4).

(2.2.4 / 2.3.3 are AAA, out of the AA set.)

### E. Genuinely semantic — oracle capture + multimodal fusion

- **Screen-reader-as-oracle** — `@guidepup/virtual-screen-reader` (pure-JS,
  CI-friendly, always-on) + real NVDA/VoiceOver via `guidepup` + the W3C **AT
  Driver** standard. Capture the spoken transcript, judge *that*. Cracks 4.1.2
  Name/Role/Value, 2.4.6 Headings & Labels, 1.3.1/1.3.2 sequence, and uniquely
  4.1.3 (announced-without-focus).
- **Multimodal fusion + Set-of-Mark prompting** — feed the model each element's
  a11y node *and* its cropped pixels; the **mismatch is the bug** (alt="logo"
  over a chart). Set-of-Mark works through any VLM today; OmniParser / UGround
  are optional self-hosted grounding sidecars (not on OpenRouter).
- **Mutation / differential testing** — strip CSS (1.3.2),
  `emulateVisionDeficiency()` greyscale (1.4.1), inject text-spacing (1.4.12),
  force zoom (1.4.4/1.4.10), toggle focus styles (2.4.7). Prior art: **Ma11y**
  (ISSTA'24, 25 mutation operators). The *delta* (Playwright `toHaveScreenshot`
  pixelmatch) is the oracle.

## Confidence and the improvement flywheel

**Asymmetric confidence (from W3C ACT outcome mapping — the most useful single
principle).** A `failed` outcome is *dispositive* (the criterion is not
satisfied). But "all passed" on a semi-automated rule means **"needs further
testing" — never a conformance pass.** So confidence should be a function of
method *and* outcome: a detected **fail** is trustworthy (can drive the red
check); a **pass** from anything short of fully-deterministic carries a
confidence ceiling and may never claim `machine_proven`.

**Estimating confidence (ranked by practicality):**
1. **Self-consistency / sampling agreement** — sample the verdict k=3–5 at
   temp>0; vote share = confidence. No logprobs needed; works on every provider.
   Black-box core of semantic entropy (Nature 2024). Highest leverage.
2. **Multi-model jury** — decorrelated failures; reserve for borderline cases;
   keep judges blind to each other (consensus bias).
3. **Verbalized confidence** — systematically overconfident (worse after RLHF);
   keep only as a recalibrated input, never the raw signal.
4. **Token logprobs** — provider-dependent/unreliable; fall back to sampling.
5. **Conformal prediction / selective classification** — abstain below a
   threshold tuned to a target risk; report risk-coverage / AUARC; calibrate
   buckets with ECE/Brier. Needs a gold set (the flywheel provides it).

**The flywheel (`collect → label → eval → improve → measure`):** auto-enqueue
every low-confidence/split-vote case (uncertainty sampling); expert review queue
(judge bootstraps, human refines → gold example; ~30–200 labels/criterion to
start); versioned golden set partitioned per criterion; improve cheapest-first
(few-shot exemplar bank → prompt optimization → distillation); measure per
criterion with Cohen's κ and **selective-accuracy-at-fixed-coverage** (aggregate
accuracy hides everything). The flywheel works when coverage rises at constant risk.

## Open product questions (not decided)

- **Delete `not_tested`** — it is already 0; out-of-scope should be
  `not_applicable` with a reason.
- **`needs_review` may not belong as a status at all** — option on the table:
  always commit pass/fail + a confidence, and route low-confidence to a review
  *queue* (an orthogonal `review_status`: auto / queued / human_confirmed /
  human_overturned). Confidence as a property of the verdict, not a third color.
- **Confidence schema** — continuous [0,1] (for curves) + a calibrated
  high/medium/low bucket (for the report)?
- **CI treatment** — keep model verdicts advisory (deterministic/scripted
  failures gate); the asymmetric rule says agentic *fails* could still warn.
- **Build order, cost in CI, and how this interacts with multi-tenant fan-out**
  (see `multi-tenant-consumers.md`) all need more design.

## Wildest bets (longer horizon)

- **Formal model-checker over the focus graph** — treat a11y tree + focus
  transitions as a state machine; prove invariants (no keyboard trap =
  strongly-connected + reachable close) → a *counterexample trace* (the exact
  trapping tab path) instead of "needs review." ACT-R rules are a seed ruleset
  to compile into Datalog.
- **Jury tournaments + distilled a11y judge** — diverse models debate the fused
  evidence; inter-rater agreement = calibrated confidence; distill into a small
  judge for the tail.
- **Blind-reconstruction alt-text metric** — give Model-A only the a11y tree +
  alt text, Model-B the screenshot; score the divergence → a reproducible
  alt-text-adequacy number for 1.1.1/1.3.3.
- **Generalized Correctness Model + user-set reliability budget** — a small
  model trained on accumulated confirmed/overturned labels outputs calibrated
  P(verdict correct); expose a knob ("auto-decide at ≤2% error → N routed to
  review"). As the gold set grows, the same tolerance buys higher coverage —
  improvement becomes visible and contractual.

## Sources

W3C ACT Rules (`w3.org/WAI/standards-guidelines/act/rules/`), ACT-Rules CG
(`act-rules.github.io`), ARIA APG + ARIA-AT, axe-core rule descriptions, Alfa
`sia-r113`/`sia-r115`, Deque coverage report, WebAIM Million, Karl Groves
testability taxonomy, Fischer (W3C WAI-GL 2019), Level Access; Apple AXNav (CHI
2024), GenA11y (FSE 2025), Ma11y (ISSTA 2024), A11y-CUA (CHI 2026); guidepup /
virtual-screen-reader, Set-of-Mark prompting, OmniParser, UGround, ScreenSpot;
Harding Flash & Pattern Analyzer, WhisperX; semantic entropy (Nature 2024), Just
Ask for Calibration (EMNLP 2023), conformal/selective-classification literature,
Generalized Correctness Models (2025).
