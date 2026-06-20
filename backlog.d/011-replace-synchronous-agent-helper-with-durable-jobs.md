# Replace synchronous agent helper with durable autonomous jobs

Priority: P0 - Status: ready - Estimate: XL

## Goal

Make Allie agent work resumable, cancellable, observable, and checkpointed so
agentic mapping and review can run for minutes or hours without being collapsed
into a fixed subprocess timeout.

## Oracle

- [ ] `allie workbench start --manifest examples/autonomous-workbench.yml --out .allie/jobs/autonomous-smoke`
  creates a durable job directory with `job.json`, `events.jsonl`, step receipts,
  artifacts, and final map/report pointers.
- [ ] `allie workbench status --job .allie/jobs/autonomous-smoke` reports
  lifecycle state, current step, last heartbeat, budget usage, and resumability.
- [ ] A long-running fixture agent step can exceed 120 seconds without being
  killed solely by wall-clock timeout, while still obeying explicit budget,
  idle, cancel, and CI policy limits.
- [ ] `allie workbench cancel` and `allie workbench resume` are covered by tests
  and leave auditable state transitions.
- [ ] Existing one-shot `map`, `report`, `review`, `remediate`, and `release`
  commands remain available as task primitives.

## Verification System

- Claim: Allie's agentic path is a durable job system, not a synchronous helper
  guarded by a hardcoded timeout.
- Falsifier: an agent run is lost on process exit, cannot be resumed, has no
  event log or heartbeat, cannot be cancelled, or still treats 120 seconds as the
  product limit.
- Driver: a deterministic long-running fixture runner plus the autonomous
  workbench smoke.
- Grader: job schema validation, event-log assertions, lifecycle transition
  tests, timeout/budget tests, and artifact presence checks.
- Evidence packet: `.allie/jobs/autonomous-smoke/` plus generated map, report,
  review, and release receipts.
- Cadence: before replacing the current runner, after each lifecycle transition,
  and before enabling real OpenCode/OMP/Codex adapters.

## Children

1. Define `allie.job.v0` with lifecycle states, step receipts, heartbeat,
   budgets, retry policy, cancel marker, resume pointer, and artifact index.
2. Add a workbench command family for `start`, `status`, `watch`, `cancel`,
   `resume`, and `await`.
3. Wrap existing `map`, `report`, `review`, `remediate`, and `release` commands
   as job steps without changing their evidence packet contracts.
4. Replace `DEFAULT_AGENT_TIMEOUT_MS` behavior with explicit runtime, idle,
   spend, model-call, tool-call, and CI-mode budget policy.
5. Add adapter contracts for OpenCode server/session mode, OMP session mode, and
   a local deterministic fixture runner.
6. Extend `npm run autonomous:smoke` to assert durable job lifecycle artifacts,
   not only static command outputs.

## Notes

**Why:** The agent runtime lane found `DEFAULT_AGENT_TIMEOUT_MS` and
`wait_timeout` in `src/lib.rs` as a one-shot advisory mapper guard. Current
agent systems support sessions, async work, abort, event streams, checkpoints,
and background execution; Allie needs that job contract before it can honestly
sell autonomous assessment.

