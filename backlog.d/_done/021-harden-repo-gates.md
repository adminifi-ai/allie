# Harden the repo's own gates

Priority: P1 · Status: done · Estimate: S

## Goal
CI enforces the same floor as local, the PUBLIC repo guards against secret
leaks, and the Rust-first codebase gets static lint.

## Oracle
- [x] CI runs the full local gate (`npm run verify`); consumer / consumer-cwd / agentic / autonomous / size smokes all run in CI (today ci.yml:19-23 runs only fmt + test + worker/evidence/release).
- [x] A gitleaks (or equivalent) secret-scan gate runs in CI.
- [x] `cargo clippy --locked -- -D warnings` runs in the gate + CI (today zero clippy enforcement anywhere).
- [x] docs/verification.md "expanded commands" are synced with `scripts/verify.sh` (missing visibility/coverage/consumer-cwd/agentic/size smokes today).

## Children
1. Make CI == local gate (`npm run verify`).
2. Add a secret-scan gate.
3. Add a clippy gate.
4. Sync docs/verification.md with verify.sh.

## Notes
**Why:** harness/security lane. The repo is clean of committed secrets today (verified by scan), but CI is *weaker* than the documented local gate, so autonomous/consumer/size regressions land green; a PUBLIC repo handling staging data + model keys has no automated secret guard; a Rust-first repo has no clippy. The size gate is genuinely ratcheted and redaction is receipted — those are good, leave them.

## Delivered

- `.github/workflows/ci.yml` already calls the repo-owned `npm run verify`
  contract; this slice keeps CI thin and strengthens that contract directly.
- `scripts/verify.sh` now runs `cargo clippy --locked -- -D warnings` and
  `npm run secrets:smoke`.
- `npm run secrets:smoke` self-tests the scanner, then scans tracked source,
  nonignored worktree files, the current commit message, and the GitHub event
  payload when present, printing only redacted matches.
- `docs/verification.md` now matches the expanded gate and documents the
  clippy, secret-scan, visibility, coverage, consumer-CWD, agentic, and
  module-size evidence surfaces.
