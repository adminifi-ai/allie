# Deepen audit autonomy depth

Priority: P1 · Status: pending · Estimate: XL

## Goal
The audit Allie autonomously produces is genuinely deep and precise — multi-step
flows, video-walkthrough vision analysis, iterative QA, multi-surface scale, and
a zero-false-positive ceiling on vision failures.

## Oracle
- [x] Generated flows include multi-step interactions/assertions, not 1:1 surface→state stubs (src/lib.rs:1009-1030, run_promote_flow:1074).
- [x] Vision model receives video walkthroughs, not only sampled stills (workers/agentic/review.mjs:244 filters to screenshots).
- [x] Agentic loop iterates (observe → act → re-judge), not single-shot (review.mjs:59-105).
- [ ] Agentic review fans out across all discovered surfaces with retries (review.mjs:51, agentic.rs:95-117).
- [ ] Vision FAIL verdicts enforce a zero-false-positive ceiling, benchmarked against a labeled set.

## Children
1. Generate multi-step interaction flows, not per-route state stubs.
2. Feed video walkthroughs to the vision model (capture exists; model judges stills only).
3. Iterative observe-act-rejudge agentic QA loop.
4. Scale agentic review across all discovered surfaces with retries.
5. Vision-verdict precision: labeled benchmark + zero-false-positive ceiling on FAILs.

## Notes
**Why:** autonomy-depth lane (flow gen is a route echo `lib.rs:1009`; video captured but stills-only `review.mjs:244`; single-shot loop `review.mjs:59`; single-surface `review.mjs:51`) + competitive lane (Deque "Advancing AI for axe" / Evinced ship vision rules at a ~zero-false-positive bar — Allie's headline must beat it). Depends on epic 015 (real surfaces) to matter. The honest-uncertainty invariant is already well enforced (`agentic.rs:271`) — preserve it.

## Progress

- 2026-06-30: Generated flow-plan candidates can now carry existing
  `flow.states[].steps`; `promote-flow` preserves them into the generated
  manifest. Local fixture discovery infers conservative deterministic steps for
  simple `aria-controls` menus and email fields, and `npm run autonomous:smoke`
  verifies the generated YAML plus post-action DOM evidence.
- 2026-06-30: Agentic model calls now include captured focus/motion walkthrough
  clips as `video_url` media alongside screenshot `image_url` parts. The
  offline `npm run agentic:smoke` gate uses a fake OpenRouter endpoint to prove
  the outgoing payload contains WebM video media without requiring a live model
  key.
- 2026-06-30: Agentic review now supports one bounded observe-act-rejudge loop:
  the model may request safe `press_key` or `wait_ms` observation actions, the
  worker captures action evidence in a fresh page, and a second model call
  re-judges within the same `max_calls` budget. The fake OpenRouter smoke proves
  the two-call loop and action-captured media without a live key.
