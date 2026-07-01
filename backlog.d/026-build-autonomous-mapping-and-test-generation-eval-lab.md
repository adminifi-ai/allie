# Build an autonomous mapping and test generation eval lab

Priority: P1 · Status: pending · Estimate: XL

## Goal
Measure and improve Allie's autonomous product-surface mapping and generated a11y test coverage against a labeled fixture corpus instead of relying on smoke success as a proxy for quality.

## Oracle
- [ ] A versioned eval corpus contains labeled routes, states, auth boundaries, workflows, expected artifacts, and known accessibility findings.
- [ ] The eval reports product-surface recall, generated-flow replay success, generated-step precision, criterion coverage, and false auth-wall capture rate.
- [ ] At least one eval fixture is derived from each dogfood class: static/public, authenticated consumer, work-management, and operations dashboard.
- [ ] Mapping misses are emitted as first-class diagnostics distinct from target accessibility failures.
- [ ] `npm run autonomous:smoke` remains the cheap regression gate, while the eval produces richer metrics for groom/release decisions.

## Verification System
- Claim: Allie's autonomous mapper and generated test planner are measurably improving on representative app shapes.
- Falsifier: The eval can pass when expected surfaces are missed, generated flows are not replayable, auth walls are audited as app states, or metric changes are inside noise with no explanation.
- Driver: A repo-owned `npm run discovery:eval` or equivalent command that runs fixed fixtures and writes `allie-discovery-eval.json`.
- Grader: Thresholded and trended recall/precision/replay metrics with fixture-by-fixture diagnostics and linked artifacts.
- Evidence packet: `.allie/evals/discovery/latest/` plus committed fixture labels under `fixtures/` or `examples/`.
- Cadence: Run the cheap slice in CI; run the larger corpus before major autonomous-discovery changes and dogfood refreshes.

## Children
1. Define the eval schema: expected surfaces, workflows, route states, auth boundaries, required artifacts, and known accessibility defects.
2. Convert current autonomous smoke fixtures into the first labeled corpus entries.
3. Add fixture snapshots inspired by Vanity, Olympus, Habitat, and Linejam without copying secrets or mutable app data.
4. Add a surface-map recall grader that compares `product-map.json` and `surface-map.html` against labels.
5. Add generated-flow replay grading for action steps, post-action state, auth markers, and blocked-login detection.
6. Add diagnostics that tell the report consumer whether a failure is an app defect, a mapping miss, a generated-test miss, or an unverified path.
7. Use the eval output to prioritize changes to `src/discovery.rs`, `src/discovery/live.rs`, and the worker adapter.

## Notes
- `docs/roadmap.md` still names authenticated staged-app discovery and changed-surface inference as future work.
- Smoke tests prove the loop runs; this epic proves the loop covers what matters.
- Do not turn the eval into model theater. Labels, replay artifacts, and comparable metrics are the product.
