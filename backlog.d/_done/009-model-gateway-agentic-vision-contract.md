# Add model gateway and agentic vision review contract

Priority: P1 - Status: done - Estimate: XL

## PRD Summary

- User: accessibility specialists who need agentic passes on criteria that
  automation cannot fully decide.
- Problem: Allie has model policy placeholders and `agentic` evidence classes,
  but no typed model gateway, no redaction receipts, no review job contract, and
  no promotion path from model hypothesis to release-relevant evidence.
- Goal: add the model and vision review contract first, with live provider calls
  disabled until policy and proof are complete.
- Why now: agentic review must be evidence-locked before live LLM integration.
- UX enabled: screenshots, videos/GIFs, DOM, a11y trees, and WCAG context can be
  reviewed by an approved vision model with traceable prompts and neutral
  findings.
- Deliverable type: schema, policy enforcement, artifacts, and fixtures.
- Success signal: model-only findings enrich reports but never block until
  promoted by scripted reproduction or human attestation.

## Product Requirements

- P0: define provider registry policy with allowlist, ZDR requirement, no silent
  fallback, spend/call/artifact budgets, prompt templates, and model metadata.
- P0: require redaction receipts before any artifact can leave the local trust
  boundary.
- P0: type `review_attempts`, `model_prompt`, `model_response`, agentic findings,
  reviewer confidence, and source artifact refs.
- P0: run offline fixture review jobs with recorded responses before enabling
  live provider calls.
- P0: preserve neutral release behavior for model-only findings.
- P0: add promotion states: `model_hypothesis`, `scripted_reproduced`,
  `human_attested`.
- P1: support vision tasks for visual order, focus visibility, target affordance,
  alt usefulness, motion concerns, and ambiguous content meaning.
- Non-goals: legal determinations, unbounded browsing, provider fallback, or
  automatic release blocking from model opinion.

## Technical Design

- Rust owns provider policy, budgets, prompt versions, review-attempt schema,
  audit events, promotion state, and release projection.
- A narrow model gateway owns provider adapters only after offline contracts pass.
- Prompts consume redacted artifact refs and WCAG-method context, not raw app
  secrets.
- Live calls require explicit policy fields and fail closed when incomplete.

## Lead Repo Read

- `SPEC.md`: agentic evidence, security/privacy, audit, and remediation
  constraints.
- `docs/architecture.md`: model gateway responsibilities.
- `docs/evidence-contract.md`: model artifact types.
- `src/lib.rs`: current disabled model policy and non-blocking release projection.
- `schemas/allie.evidence.v0.schema.json`: untyped review field.

## Deliverable

- Output: typed model policy, review-attempt schema, redaction receipt schema,
  offline review fixtures, neutral release tests, and promotion fixtures.
- Acceptance oracle: fixture packets with recorded model responses generate
  agentic findings and report drilldown context; release remains neutral until
  a scripted or human-backed promotion fixture exists.
- Evidence artifacts: prompt/response artifacts, redaction receipts, audit log,
  report drilldown, and release summary.
- Residual risk: live provider quality and privacy claims remain unproven until
  explicitly enabled and tested with approved credentials.

## Verification System

- Claim: Allie can use vision AI without turning model opinion into unearned
  compliance status.
- Falsifier: live call runs with incomplete policy; provider fallback occurs;
  unredacted artifact leaves gateway; model-only finding blocks release; prompt
  or response is missing metadata; or promoted finding lacks scripted/human proof.
- Driver: recorded provider fixtures and promotion-state packet fixtures.
- Grader: schema validation, fail-closed policy tests, redaction receipt tests,
  release projection tests, and report snapshots.
- Evidence packet: review-attempt fixtures plus generated report/release outputs.
- Cadence: before any live provider integration.

## Children

1. Type model policy and provider registry config.
2. Define redaction receipt and review-attempt schemas.
3. Add offline prompt/response fixture ingestion.
4. Render agentic context in WCAG drilldown reports.
5. Add neutral release tests for model-only findings.
6. Add scripted/human promotion fixtures and release projection tests.

## Notes

The model gateway is a compliance boundary, not a convenience wrapper. It should
be impossible to confuse an agentic observation with a release-blocking finding
unless Allie has replay or human-attested proof.

## Delivered

- Added `allie review --packet <evidence.json> --out <dir>`.
- Added offline-recorded model review attempts with prompt artifacts, response
  artifacts, redaction receipt artifacts, provider/model metadata, confidence,
  and `model_hypothesis` promotion state.
- Agentic findings enrich reviewed packets but stay non-blocking in release
  projection unless promoted by scripted or human-backed proof.
- Live provider calls remain intentionally disabled; this delivery implements
  the evidence-locked gateway contract and fixture review path first.
- Verified by `cargo test --locked` neutral-release assertions and
  `npm run autonomous:smoke`, which writes `.allie/reviews/autonomous-smoke/`.
