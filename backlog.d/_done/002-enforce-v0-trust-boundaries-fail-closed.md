# Enforce V0 trust boundaries fail-closed

Priority: P0 - Status: done - Estimate: L

## Goal

Make credentials, sensitive artifacts, worker failures, model-disabled policy,
and deterministic exit classes explicit enough that incomplete or unsafe
evidence cannot masquerade as a valid release packet.

## Oracle

- [x] Evidence packets identify credential profile names without storing secret
  values.
- [x] Artifact metadata records hash, redaction status, retention class, and
  withheld/unavailable reasons.
- [x] Worker timeout, crash, missing credential, unreachable target, axe failure,
  partial write, and nondeterminism map to stable packet statuses and exit
  classes.
- [x] Model-call fields remain disabled by default and fail closed if provider
  policy is incomplete.

## Verification System

- Claim: V0 fails closed across the trust boundaries that can leak data or
  corrupt release decisions.
- Falsifier: a fixture can create a packet with raw secret values, unredacted
  sensitive artifact metadata marked safe, missing failure classification, or
  model-provider fallback.
- Driver: trust-boundary fixtures for credential profiles, artifact redaction
  states, worker failures, and disabled model policy.
- Grader: packet assertions, exit-code assertions, and absence of secret values
  in packet/report artifacts.
- Evidence packet: `.allie/runs/trust-boundary-*` fixture outputs.
- Cadence: after the V0 local loop exists and before real staging auth or model
  enrichment is enabled.

## Children

1. Define credential provider shape, credential profile naming, and auth-state
   artifact rules.
2. Define redaction profile semantics, raw-vs-redacted storage rules, and
   retention classes.
3. Add artifact integrity fields: hash, creation tool, related state, and
   redaction/withheld status.
4. Define worker failure and nondeterminism taxonomy with packet status and
   exit-code mapping.
5. Reserve model audit fields while keeping provider calls disabled by default.

## Notes

**Why:** The security/privacy lane flagged credentials, artifact capture, and
runtime failure semantics as the highest-risk V0 trust boundaries. `SPEC.md`
already requires scoped credentials, redaction, no silent provider fallback,
audit logs, and bounded exploration; this ticket turns those requirements into
testable packet behavior.

## Delivered

- Added manifest credential, artifact, model, timeout, and known-nondeterminism policy fields.
- Added packet metadata for credential provider status, artifact redaction/retention/unavailable state, infrastructure failure counts, and stable `failure_class`.
- Added fail-closed packet/exit handling for missing credentials, incomplete model policy, worker crash/error/timeout/partial write, unreachable target, missing required artifacts, axe failures, and nondeterminism.
- Added negative fixtures: `examples/trust-missing-credential.yml`, `examples/trust-model-policy-incomplete.yml`, and `examples/trust-unreachable-target.yml`.
- Verified with `npm run verify`, serial trust-boundary fixture runs, and secret scan `rg -n "super-secret|secret-value|password|token" .allie/runs/trust-boundary-*` returning no matches.
