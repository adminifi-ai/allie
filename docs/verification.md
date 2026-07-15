# Allie Verification Runbook

This runbook is the cold operator path for verifying Allie without chat context.

## Setup

Install browser-worker dependencies once per checkout:

```sh
npm ci
npx playwright install chromium
```

In a consuming repository, install the release bundle and put its `bin`
directory on `PATH`. The bundle keeps the Rust binary and browser worker assets
together, so the CLI resolves the worker automatically.

```sh
mkdir -p .allie/tooling
curl -fsSL https://github.com/adminifi-ai/allie/releases/latest/download/allie-linux-x64.tar.gz \
  | tar -xz -C .allie/tooling
export PATH="$PWD/.allie/tooling/allie/bin:$PATH"
allie doctor --manifest .allie/manifest.yml --out .allie/doctor
allie verify --manifest .allie/manifest.yml --out .allie/verify/latest
allie publication --verify-root .allie/verify/latest --out .allie/public/latest
```

For source-checkout development, run `npm ci` and `npx playwright install
chromium` in the Allie checkout once. `ALLIE_BROWSER_WORKER` is still an
explicit override for nonstandard layouts, not part of the normal consumer path.
Release bundles are built with `npm run package:release`; tag pushes publish the
Linux bundle consumed by the CI examples.

Treat `.allie/verify/latest` as sensitive local evidence. It can contain
authenticated DOM, screenshots, accessibility trees, traces, prompts, URLs,
console/network details, and axe HTML. Public CI publishers must run
`allie publication` and archive only the four allowlisted files under
`.allie/public/latest`. The projection
contains count-level public summaries and a policy receipt; it intentionally
does not preserve the private HTML drilldown. Keep the canonical verify tree in
policy-approved private storage when accessibility engineers need its full
evidence depth.

A successful projection contains four public-safe files:

```text
.allie/public/latest/allie-public-summary.json
.allie/public/latest/allie-public-summary.md
.allie/public/latest/publication-receipt.json
.allie/public/latest/allie-run-manifest.json
```

Requesting any file from the canonical verify tree with `--include` is a
retryable policy refusal (exit `2`). The refusal receipt remains in the public
output directory; the sensitive source evidence remains local and unchanged.

The repository CI workflow is intentionally thin: `.github/workflows/ci.yml`
installs Rust, Node, npm dependencies, and Playwright, then calls the same
repo-owned `npm run verify` contract operators run locally.

## Gate

Run the full local gate:

```sh
npm run verify
```

Equivalent expanded commands:

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked -- -D warnings
npm run secrets:smoke
npm run landscape:smoke
npm run worker:smoke
npm run evidence:smoke
npm run axe-rules:smoke
npm run action:smoke
npm run auth:smoke
npm run visibility:smoke
npm run coverage:smoke
npm run consumer:smoke
npm run consumer-cwd:smoke
npm run distribution:smoke
npm run agentic:smoke
npm run agentic:precision
npm run release:smoke
npm run autonomous:smoke
npm run size:smoke
```

## Expected Evidence

`cargo clippy --locked -- -D warnings` keeps the Rust-first core warning-free
under the same lockfile as the tests.

`npm run secrets:smoke` runs the repo-owned secret scan self-test and then scans
tracked source, nonignored worktree files, textual generated evidence under
`.allie/`, the current commit message, and the GitHub event payload when
present. Heavy tooling/browser bundles and binary captures are skipped;
publication policy rejects binary screenshots and all other canonical evidence
instead of treating absence of a textual secret match as approval. Findings are
printed with redacted matches.

`npm run landscape:smoke` validates the competitive record shape, review age,
approved source set, and roadmap linkage that the landscape document claims are
machine-checked.

`npm run worker:smoke` proves the Node worker can run Playwright plus axe. It
leaves:

```text
.allie/worker-smoke/worker-request.json
.allie/worker-smoke/worker-response.json
.allie/worker-smoke/artifacts/axe-login-form.json
.allie/worker-smoke/artifacts/login-form.png
.allie/worker-smoke/artifacts/axe-mobile-login-form.json
.allie/worker-smoke/artifacts/mobile-login-form.png
```

`npm run evidence:smoke` proves the Rust CLI, worker, packet writer, and report
writer work together. The smoke freezes the run clock with `SOURCE_DATE_EPOCH`,
runs the fixture once on an OS-assigned ephemeral port (so concurrent gate
runs never collide on a fixed port), pins the second run to that same
discovered port, and byte-compares the resulting `evidence.json` and
`report.html`. It
also asserts the packet schema, pass summary, non-empty Git provenance, stable
fixture URL, captured state metadata, artifact hashes, distinct confidence
classes, explicit `needs_review` and `not_applicable` verdicts, and the report's
status/replay sections. It also asserts mobile-web viewport metadata plus
mobile screenshot and axe artifacts for the captured state. It leaves:

```text
.allie/runs/v0-smoke/evidence.json
.allie/runs/v0-smoke/report.html
.allie/runs/v0-smoke/artifacts/axe-login-form.json
.allie/runs/v0-smoke/artifacts/login-form.png
.allie/runs/v0-smoke/artifacts/axe-mobile-login-form.json
.allie/runs/v0-smoke/artifacts/mobile-login-form.png
```

The expected happy-path packet summary is `status: pass`, `exit_code: 0`,
one captured state, and artifact types `axe_json`, `screenshot`, and
`html_report`, with the mobile-web pass represented as additional `axe_json`
and `screenshot` state artifacts.

`npm run visibility:smoke` proves a captured packet can be mapped and rendered
into the product-map, surface-map, WCAG report, and markdown summary visibility
artifacts:

```text
.allie/maps/v0-smoke/product-map.json
.allie/maps/v0-smoke/surface-map.html
.allie/maps/v0-smoke/agent-runner-receipt.json
.allie/maps/v0-smoke/generated-flow.yml
.allie/reports/v0-smoke/compliance-report.json
.allie/reports/v0-smoke/compliance-report.html
.allie/reports/v0-smoke/summary.md
```

`npm run action:smoke` proves manifest state setup actions execute before
evidence capture. It clicks open the workbench fixture menu, waits for the panel,
asserts the DOM artifact contains the revealed link, then runs a negative
control where a missing action selector records `state-step-failed` and blocks:

```text
.allie/runs/action-steps-smoke/evidence.json
.allie/runs/action-steps-smoke/report.html
.allie/runs/action-steps-smoke/worker-request.json
.allie/runs/action-steps-smoke/artifacts/dom-open-menu.html
.allie/runs/action-steps-smoke/artifacts/dom-typed-email.html
.allie/runs/action-steps-negative-smoke/evidence.json
```

`npm run auth:smoke` proves authenticated coverage through both `allie run` and
the composed `allie verify` pipeline. It logs into the local auth fixture with
env-sourced credentials, reaches the gated dashboard without writing the
credential value to artifacts, proves the storageState hatch reaches the same
route without writing the storageState path to artifacts, and runs negative
controls that block on `auth-lost` instead of treating the HTTP-200 login wall
as covered app content:

```text
.allie/runs/auth-smoke/evidence.json
.allie/runs/auth-smoke-storage-state/evidence.json
.allie/runs/auth-smoke-neg/evidence.json
.allie/verify/auth-smoke/run/evidence.json
.allie/verify/auth-smoke/reporters/allie-report.json
.allie/verify/auth-smoke-storage-state/run/evidence.json
.allie/verify/auth-smoke-storage-state/reporters/allie-report.json
.allie/verify/auth-smoke-neg/run/evidence.json
.allie/verify/auth-smoke-neg/reporters/allie-report.json
```

`npm run coverage:smoke` proves the WCAG 2.2 AA obligation ledger stays a
55-success-criterion surface matrix, keeps supporting checks out of the
denominator, exposes a `wcag21-aa` profile view for EAA/EN 301 549 consumers,
and requires provenance on terminal cells. The `wcag21-aa` view has a
50-criterion denominator, excludes WCAG 2.2-only criteria such as 2.5.8 Target
Size (Minimum), and explicitly records the WCAG 2.1-only 4.1.1 Parsing legacy
gap instead of silently counting it covered:

```text
.allie/runs/coverage-matrix-smoke/evidence.json
.allie/reports/coverage-matrix-smoke/compliance-report.json
.allie/reports/coverage-matrix-smoke/compliance-report.html
.allie/reports/coverage-matrix-smoke/summary.md
```

`npm run consumer:smoke` proves the portable consuming-app contract. It
scaffolds a manifest with `allie init`, runs `allie verify` over the same
manifest, checks that GitHub and Azure examples call the same CLI command, and
leaves:

```text
.allie/consumer-contract-smoke/manifest.yml
.allie/consumer-contract-smoke/discovery/discovery.json
.allie/consumer-contract-smoke/flow/generated-flow.yml
.allie/consumer-contract-smoke/map/product-map.json
.allie/consumer-contract-smoke/run/evidence.json
.allie/consumer-contract-smoke/report/compliance-report.json
.allie/consumer-contract-smoke/report/compliance-report.html
.allie/consumer-contract-smoke/release/release-summary.json
.allie/consumer-contract-smoke/reporters/allie-report.json
.allie/consumer-contract-smoke/reporters/allie-compliance-report.json
.allie/consumer-contract-smoke/reporters/allie-report.html
.allie/consumer-contract-smoke/reporters/allie-report.md
.allie/consumer-contract-smoke/reporters/junit.xml
.allie/consumer-contract-smoke/reporters/allie.sarif
```

`npm run consumer-cwd:smoke` proves the installed/debug binary can run from a
foreign consumer repository working directory while still handing the browser
worker absolute request, response, and artifact paths. It uses a temporary
consumer Git checkout and expects one captured state, zero infrastructure
failures, and non-empty packet provenance from that checkout instead of the
Allie tool checkout.

`npm run release:smoke` proves the packet can drive release decisions without a
second status model. It reads `.allie/runs/v0-smoke/evidence.json` and leaves:

```text
.allie/releases/v0-smoke/release-summary.json
.allie/releases/v0-smoke/github-check.json
.allie/releases/v0-smoke/release-report.html
```

The expected V0 fixture decision is `needs_review` with a neutral GitHub check:
deterministic evidence passed, but keyboard/focus/zoom/reduced-motion and human
assistive-technology obligations are still marked `not_tested` or
`needs_review`.

`npm run agentic:smoke` proves the agentic review gateway launches the browser,
captures media, and returns a well-formed inconclusive assessment when no model
API key is present, with a `not_sent` model-redaction receipt. Before capture it
also refuses missing or unsupported redaction modes and proves the fake
provider receives no request. It then starts a local fake OpenRouter endpoint and verifies
that a model request includes both screenshot `image_url` parts and captured
WebM walkthrough clips as `video_url` parts only under explicit `none`, retaining
a truthful `not_applied` receipt. The fake endpoint reviews the
workbench home and settings surfaces, first requests a bounded `press_key`
observation on home, then forces a transient settings model failure so the
gateway must retry inside the same call budget. The final fake verdict fails
because settings fails, proving fail precedence across surfaces. This keeps the
smoke offline while locking the provider payload shape, observe-act-rejudge
loop, multi-surface fan-out, and retry behavior; live model behavior is covered
by real `verify` and workbench runs.

`npm run agentic:precision` proves vision FAIL promotion has a zero-false-
positive ceiling. It runs the agentic worker against labeled workbench surfaces
through a fake OpenRouter endpoint. One scenario intentionally labels both
surfaces as expected pass while the fake model returns a settings FAIL; the
worker must emit `precision_gate.status: fail` with one false-positive FAIL. A
second scenario labels settings as expected fail and home as expected pass; the
worker must emit `precision_gate.status: pass` with zero false positives. Rust
uses that gate when ingesting live agentic responses: PASS can promote, but
FAIL promotes only when `precision_gate` passed with at least one expected-pass
labeled case and zero false-positive FAILs. Without that proof, the FAIL stays
attached as review context and the criterion remains `needs_review`.

`npm run autonomous:smoke` proves the autonomous workbench path. It leaves:

```text
.allie/jobs/autonomous-smoke/job.json
.allie/jobs/autonomous-smoke/events.jsonl
.allie/jobs/autonomous-smoke/steps/discovery/discovery.json
.allie/jobs/autonomous-smoke/steps/map/product-map.json
.allie/jobs/autonomous-smoke/steps/run/evidence.json
.allie/jobs/autonomous-smoke/steps/report/compliance-report.json
.allie/jobs/autonomous-smoke/steps/release/release-summary.json
.allie/jobs/autonomous-agentic-smoke/steps/run/agentic-request.json
.allie/jobs/autonomous-agentic-smoke/steps/run/agentic-response.json
.allie/jobs/autonomous-agentic-smoke/steps/run/evidence.json
.allie/jobs/autonomous-agentic-smoke/steps/report/compliance-report.json
.allie/jobs/autonomous-agentic-smoke/steps/release/release-summary.json
.allie/jobs/autonomous-agentic-error-smoke/job.json
.allie/jobs/autonomous-agentic-error-smoke/events.jsonl
.allie/discovery/autonomous-smoke/discovery.json
.allie/discovery/autonomous-smoke/flow-plan.json
.allie/discovery/autonomous-smoke/generated-flow.yml
.allie/runs/autonomous-smoke/evidence.json
.allie/releases/autonomous-smoke/release-summary.json
```

The smoke expects the generated replay to find the fixture's known deterministic
contrast issue. That failure proves enforcement is wired: with the model
disabled, the review step passes the replay evidence packet through
unchanged (no fabricated agentic finding), and release projection blocks on
deterministic evidence.

The generated flow is not only a route echo. Its flow-plan candidates and
promoted manifest include deterministic state steps for the workbench fixture:
the home state clicks the actions menu and waits for `#menu:not([hidden])`, while
the settings state fills and types into `#email` before waiting for
`#email-preview[data-ready]`. The smoke checks both the generated YAML and the
captured DOM artifacts so a future route-only regression fails.

Use the job status command to inspect the durable run state:

```sh
cargo run --locked -- workbench status --job .allie/jobs/autonomous-smoke
```

The expected completed fixture job has `schema: allie.job.v0`, `status:
blocked`, release status `blocked`, resumable state, and artifact pointers for
map, evidence, compliance report, review, and release outputs. The smoke also
verifies that `workbench start` refuses to reuse an existing job directory,
that non-local advisory agent modes stay on the one-shot `map` path until
durable session adapters exist, and that a `model.enabled` workbench job runs
the live agentic gateway before report/release. The model-enabled smoke unsets
the configured API key, so the gateway must still capture media and write
degraded `inconclusive` assessments without fabricating pass/fail verdicts. It
also inspects `agentic-request.json` to prove the live request carries both
home and settings review surfaces from the evidence packet, and checks that the
resulting agentic assessment retains settings-surface media. A separate
synthetic worker-error path proves agentic worker infrastructure failures fail
the `review` step before report/release instead of being recorded as completed
advisory review.

`npm run size:smoke` enforces the Rust module-size ratchet. In the canonical
checkout it asks Git for every tracked and nonignored untracked `*.rs` file,
including nested modules such as `src/discovery/*.rs` and
`src/workbench/tests.rs`. Ignored build output, caches, and nested worktrees do
not contaminate the scan. Each file's cap is recorded in
`scripts/module-size-caps.tsv`, keyed by
its path relative to the repo root; an unlisted file falls back to the
script's `DEFAULT_CAP`. Shrinking a file is free — lower its recorded cap to
lock the win in. Growing past a cap fails the gate; extract a cohesive module
instead of raising the cap, and never raise `DEFAULT_CAP` or a cap silently.
The gate resolves the repo root from its own script location (not the
caller's CWD) and treats scanning zero files as a hard failure, never a
silent pass. `scripts/module-size-gate.sh --self-test` exercises this
directly: nonignored untracked files, ignored nested worktrees, filenames
containing spaces, invocation from an unrelated working directory, an empty
scan root, and a file whose final line lacks a trailing newline (which plain
`wc -l` would undercount) all have dedicated checks against injected,
cleaned-up fixture files.

## Failure Meanings

- Exit `0`: evidence passed for the required deterministic/scripted checks.
- Exit `1`: deterministic or scripted required evidence failed, such as an axe
  violation, HTTP route-state failure, or missing required artifact.
- Exit `2`: infrastructure or trust-boundary failure, such as missing
  credentials, incomplete model policy, worker crash, timeout, unreachable
  target, partial worker response, or nondeterminism.
- Exit `64`: CLI usage error.

For `allie release`, exit `1` means a packet failure, missing changed-surface
evidence, expired touched waiver, or invalid touched-waiver metadata blocked the
release decision. Stale evidence and model-only findings are review-required
neutral outputs, not hard release blocks.

For `allie verify`, `approved` and `needs_review` exit `0`; `blocked` exits `1`
because required evidence blocks the projection; infrastructure failures exit
`2`.

## Trust-Boundary Fixtures

These are negative verification fixtures:

```sh
cargo run --locked -- run --manifest examples/trust-missing-credential.yml --out .allie/runs/trust-boundary-missing-credential
cargo run --locked -- run --manifest examples/trust-model-policy-incomplete.yml --out .allie/runs/trust-boundary-model-policy
cargo run --locked -- run --manifest examples/trust-unreachable-target.yml --out .allie/runs/trust-boundary-unreachable
```

Each should exit `2` and write an `error` packet explaining the failure class.

## Release Projection

Project any packet into release/check outputs:

```sh
cargo run --locked -- release --packet .allie/runs/v0-smoke/evidence.json --out .allie/releases/v0-smoke --changed-surface login-form
```

Use one `--changed-surface <id>` per touched surface. The release projection
matches each changed surface against packet `coverage.states_captured` and
`coverage.surfaces_discovered`.

## Cleanup

Generated receipts live under `.allie/` and are ignored by Git. Remove them
when they are no longer needed:

```sh
rm -rf .allie
```
