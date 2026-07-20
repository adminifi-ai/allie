# Allie

Allie is a Rust-first accessibility evidence harness and release intelligence
system.

Licensed under the [Apache License 2.0](LICENSE).

The product goal is not to be another accessibility scanner or an "AI fixes
accessibility" overlay. Allie's moat is the artifact a release can carry: a
replayable evidence packet, a complete WCAG obligation ledger, and a
criterion-by-surface report tied to the code revision that produced it.

Positioning:

> Accessibility evidence for every release.

For the full north star — what Allie is, who it's for, the strategic bets, and what excellent looks like — see [`VISION.md`](VISION.md).

## Why This Exists

Accessibility work is often split across manual expert review, browser extensions, point-in-time axe runs, ad hoc screenshots, and release conversations that are hard to reproduce. Allie turns that into an evidence system:

- deterministic checks where machines can be certain;
- scripted browser flows where interaction behavior matters;
- screenshots, video, DOM, and accessibility tree artifacts where human or agent review needs context;
- standards-mapped status across WCAG, ADA, Section 508, and client policy packs,
  with WCAG results shown as an obligation ledger instead of a global score;
- PR and release gates that block real regressions without pretending uncertain findings are certain.

Why not an overlay? The FTC's April 2025 final accessiBe order required a
$1 million payment and barred unsupported claims that automated products can
make or keep websites WCAG-compliant. Allie takes the opposite trust posture:
it reports evidence, status, confidence, replay commands, and residual review
needs. It does not promise legal compliance, lawsuit protection, or automated
remediation. Sources: [FTC final order](https://www.ftc.gov/news-events/news/press-releases/2025/04/ftc-approves-final-order-requiring-accessibe-pay-1-million)
and [FTC case page](https://www.ftc.gov/legal-library/browse/cases-proceedings/2223156-accessibe-inc).

## Initial Shape

- Rust CLI and orchestrator.
- Node/Playwright/axe worker boundary for browser automation.
- Evidence packets as the durable contract.
- Model gateway for multimodal first-pass review, with OpenRouter only behind strict privacy and provider-routing policy.
- Local HTML/JSON reports first; hosted dashboards later.

## Consumer Contract

Allie's consuming-app contract is local and host-agnostic first:

```sh
allie init --manifest .allie/manifest.yml --app-name "My App"
allie doctor --manifest .allie/manifest.yml --out .allie/doctor
allie verify --manifest .allie/manifest.yml --out .allie/verify/latest
allie publication --verify-root .allie/verify/latest --out .allie/public/latest
```

`allie init` writes a minimal manifest without assuming GitHub, Azure, or a
hosted dashboard. By default it points at `http://127.0.0.1:3000`; pass
`--fixture-dir <dir>` when the target is a checked-in static fixture.

For `target.kind: web` manifests, discovery starts at `target.base_url` and
performs a bounded same-origin HTML link crawl plus same-origin `/sitemap.xml`
route discovery before generating the flow plan and product map. The current
live crawler supports `http://` targets and static HTML routes; HTTPS/TLS
crawling, authenticated crawling, JavaScript-driven navigation discovery, and
model review depth are separate follow-on layers. Manifest-listed states remain
authoritative and merge with routes found from live links. When a live route or
sitemap cannot be fetched, Allie keeps manifest-declared states and records the
miss in discovery diagnostics instead of silently implying complete coverage.

`allie verify` is the primary operator command for consuming apps. It composes
the existing discovery, generated-flow, product-map, evidence-run, WCAG report,
and release-projection primitives, then writes stable reporter files:

```sh
.allie/verify/latest/reporters/allie-report.json
.allie/verify/latest/reporters/allie-compliance-report.json
.allie/verify/latest/reporters/allie-report.html
.allie/verify/latest/reporters/allie-report.md
.allie/verify/latest/reporters/junit.xml
.allie/verify/latest/reporters/allie.sarif
```

Every `--out` directory (`run`, `report`, `release`, `verify`, `publication`) describes
exactly one run. A small `allie-run-manifest.json` marks the directory as
owned by that command: it is written with `phase: "in_progress"` when the
run starts and rewritten with `phase: "complete"` plus the full file list
when the run finishes, so a crashed run stays visible and never wedges the
directory. A rerun into a manifested directory removes everything in it
before writing fresh output — stale artifacts from older runs or retired
code paths, partial output from a crashed run, and any files you dropped in
by hand are all absorbed and cleaned. Treat managed out-dirs as Allie's,
not scratch space. A directory that already has content and no
`allie-run-manifest.json`, or whose manifest belongs to a different
command, is refused outright with nothing deleted — point `--out` at a
fresh or empty directory rather than one Allie cannot account for.

GitHub and Azure examples live in [docs/ci](docs/ci). They call the same
`allie verify` command, then run `allie publication` and upload only its four
allowlisted files from `.allie/public/latest`. The canonical `.allie/verify/latest` tree is sensitive
local evidence by default: it can contain authenticated DOM, screenshots,
accessibility trees, traces, prompts, URLs, console/network details, and axe
HTML. `allie publication` emits a deliberately small `public_summary`
projection plus a policy receipt. A request to include any canonical evidence
is refused as `sensitive_local`, leaves the local tree unchanged, and produces
a retryable refusal receipt. Receipts are themselves `public_summary` artifacts
and never echo a refused raw path. Host-specific files do not fork that policy.

Arbitrary repositories install the release bundle, then run the same local
preflight and verification commands. The bundle layout keeps the Rust binary and
browser worker assets together, so the CLI resolves the worker automatically.
Download the archive, checksum manifest, and adjacent Sigstore bundle first;
verify both the checksum and the GitHub Actions signer before extracting any
release content:

```sh
set -eu
release=v0.1.0
archive=allie-linux-x64.tar.gz
base="https://github.com/adminifi-ai/allie/releases/download/$release"
download=.allie/tooling/download
mkdir -p "$download" .allie/tooling
curl -fsSLo "$download/$archive" "$base/$archive"
curl -fsSLo "$download/SHA256SUMS" "$base/SHA256SUMS"
curl -fsSLo "$download/$archive.sigstore.json" "$base/$archive.sigstore.json"
(
  cd "$download"
  checksum_entries=$(awk -v archive="$archive" '$2 == archive { count++ } END { print count + 0 }' SHA256SUMS)
  if [ "$checksum_entries" -ne 1 ]; then
    printf 'expected exactly one checksum for %s, found %s\n' "$archive" "$checksum_entries" >&2
    exit 1
  fi
  awk -v archive="$archive" '$2 == archive' SHA256SUMS | sha256sum --check -
)
cosign verify-blob \
  --bundle "$download/$archive.sigstore.json" \
  --certificate-identity "https://github.com/adminifi-ai/allie/.github/workflows/release.yml@refs/tags/$release" \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  "$download/$archive"
tar -xzf "$download/$archive" -C .allie/tooling
export PATH="$PWD/.allie/tooling/allie/bin:$PATH"
allie doctor --manifest .allie/manifest.yml --out .allie/doctor
allie verify --manifest .allie/manifest.yml --out .allie/verify/latest
allie publication --verify-root .allie/verify/latest --out .allie/public/latest
```

When working from a source checkout instead of a release bundle, run `npm ci`
and `npx playwright install chromium` in the Allie checkout once. The
`ALLIE_BROWSER_WORKER` override remains available only for nonstandard layouts.
Landmark manages Allie's own pre-stable release line: after `ci` succeeds on
`master`, the pinned Landmark CLI computes the next `v0.x` version and generates
the technical changelog plus user-facing Markdown, text, HTML, JSON, and RSS
release notes. The release-intelligence workflow uses the dedicated release app
to synchronize the Rust and browser-worker versions on a generated release
branch and open a pull request. The normal `verify` check protects that merge;
after the merged commit's own CI succeeds, the same app tags that exact commit
without bypassing `master`. The authenticated tag push triggers the bundle
workflow. Landmark is development/release infrastructure for Allie
itself; the `allie` CLI never invokes Landmark while auditing a target
repository.

Release bundles are produced with `npm run package:release`. The tag workflow
builds and verifies without write or OIDC privileges, then passes only the
named archive, checksum manifest, and tag-matched Landmark notes to a minimal
signing/publishing job. That job creates a draft, uploads the three exact
expected assets, reads their names back through the GitHub API, and only then
publishes; any failed draft is deleted.

RustSec ignores live in `.cargo/audit.toml`; every ignored advisory must have
one matching structured record in `.cargo/audit-waivers.toml` with
`tracking_ref`, `rationale`, `owner`, a future TOML calendar `expiry`, and the
`removal` condition. CI rejects malformed, expired, duplicate, undocumented,
or unreferenced waiver records.

Interpret results as evidence status: `approved` exits `0`, `needs_review`
exits `0` with neutral review-required evidence, `blocked` exits `1` because
deterministic or required evidence blocks release, and infrastructure failures
exit `2`.

## Repository Map

- [VISION.md](VISION.md): project north star — intent, invariants, and what excellent looks like.
- [SPEC.md](SPEC.md): product contract and acceptance model.
- [docs/architecture.md](docs/architecture.md): proposed system design.
- [docs/evidence-contract.md](docs/evidence-contract.md): first evidence schema shape.
- [docs/naming.md](docs/naming.md): naming decisions and alternates.
- [docs/roadmap.md](docs/roadmap.md): proposed build sequence.

## V0 Local Evidence Loop

```sh
cargo run --locked -- run --manifest examples/login-flow.yml --out .allie/runs/latest
```

The V0 command runs the checked-in login fixture through the browser worker,
captures axe output and a screenshot, and writes a replayable evidence packet
plus a local HTML report:

```sh
.allie/runs/latest/evidence.json
.allie/runs/latest/report.html
.allie/runs/latest/artifacts/axe-login-form.json
.allie/runs/latest/artifacts/login-form.png
```

The packet reports accessibility evidence status, confidence, and residual
review needs. It is not a legal compliance guarantee.

Each listed flow state may include non-secret setup actions that run after
navigation and before evidence capture. Use these for deterministic UI state,
such as opening a menu, filling a search box with fixture text, or waiting for a
panel before axe, screenshot, DOM, accessibility-tree, video, trace, or keyboard
evidence is captured:

```yaml
flow:
  states:
    - id: opened-menu
      path: /
      steps:
        - click: { selector: "#open-menu" }
        - wait_for: { selector: "#menu:not([hidden])" }
      axe: true
      screenshot: true
      dom_snapshot: true
```

State-step literal values are serialized into the worker request and may appear
in artifacts. Keep credentials and other secrets in `auth.steps` with
`value_env`; use `flow.states[].steps` only for non-secret interaction setup.

## Authenticated Audit

To audit routes behind a login wall, add an optional `auth` block to the
manifest. Allie establishes a session once per run, then audits the gated routes
listed in `flow.states`.

```yaml
credentials:
  profile: my-app-auth
  provider: env
  env: ALLIE_APP_PASSWORD      # the credential value is read from the environment
  required: true
auth:
  start_path: /login           # where the login recipe begins
  steps:
    - fill:  { selector: "#email",            value_env: ALLIE_APP_USER }
    - fill:  { selector: "#password",         value_env: ALLIE_APP_PASSWORD }
    - click: { selector: "button[type=submit]" }
    - wait_for: { selector: "#dashboard" }     # success signal: selector OR url_contains
  authenticated_marker: { selector: "#dashboard" }  # asserted on every gated state
  # storage_state_env: ALLIE_APP_STORAGE_STATE       # optional SSO hatch (see below)
```

Two invariants make this trustworthy:

- **Secrets never persist.** A `fill` step carries only the env-var *name*
  (`value_env`), never a value. The browser worker reads the secret value from
  its own inherited environment at run time. No credential value is written to
  `worker-request.json`, the evidence packet, or any artifact.
- **No silent gaps.** `authenticated_marker` is asserted after navigation on
  every gated state. If the marker is absent (for example a session was lost and
  the app bounced back to the login wall — even at HTTP 200), the run records an
  `auth-lost` finding and **blocks** with a non-zero exit. A login wall is never
  audited as if it were the app.

For SSO/OAuth flows that a recipe cannot drive, use the `storage_state_env`
escape hatch: capture a Playwright `storageState` JSON out of band, point an env
var at its path, and Allie loads the session from that file instead of running
the login steps.

Run the authenticated-audit smoke (no real secret required) with:

```sh
npm run auth:smoke
```

It logs into a local fixture, reaches the gated route, verifies the credential
value never lands in any artifact, proves the `storage_state_env` hatch reaches
the same gated route from a captured session, and confirms the negative control
(no session → HTTP-200 login wall) blocks instead of passing.

## WCAG Coverage Report

```sh
cargo run --locked -- report \
  --map .allie/maps/latest/product-map.json \
  --packet .allie/runs/latest/evidence.json \
  --out .allie/reports/latest
```

For the `wcag22-aa` profile, the compliance report uses exactly the 55 WCAG
2.2 A/AA success criteria as the standards denominator. Allie support checks
such as deterministic axe aggregate status, keyboard traversal, zoom/reflow,
reduced-motion, agentic review, and human-review aggregate context are reported
separately and linked into criterion cells as evidence; they are not counted as
extra WCAG standards.

The report also includes a `wcag21-aa` profile view for EAA/EN 301 549 readers.
That view is derived from the same WCAG 2.2 ledger: WCAG 2.2-only criteria are
excluded, while the WCAG 2.1-only `4.1.1 Parsing` criterion is exposed as an
explicit legacy gap rather than silently counted as covered. The European
Commission currently describes harmonized EN 301 549 v3.2.1 as WCAG 2.1-based
and notes that later WCAG versions do not automatically become legally relevant
until included in a harmonized standard referenced in the Official Journal:
[EC standards and harmonisation](https://digital-strategy.ec.europa.eu/en/policies/web-accessibility-directive-standards-and-harmonisation).

Each required product surface/state receives a criterion matrix cell with
status, applicability, method, confidence, evidence refs, agentic refs, waiver
refs, and residual review need. Terminal claims (`pass`, `fail`, `waived`, or
`risk_accepted`) require provenance. Empty cells stay `not_tested` or
`needs_review` instead of implying compliance.

Mobile web is in scope. The browser worker records a mobile viewport evidence
pass for captured web states, including mobile screenshot and axe artifacts.
Mobile-relevant WCAG criteria such as 1.3.4 Orientation, 1.4.10 Reflow, 2.5.1
Pointer Gestures, 2.5.4 Motion Actuation, and 2.5.8 Target Size remain in the
ledger at mobile viewports; unresolved judgment stays `needs_review` with the
mobile evidence attached. Native mobile apps are outside Allie's current V0 web
target scope.

## Release Decision Projection

```sh
cargo run --locked -- release --packet .allie/runs/latest/evidence.json --out .allie/releases/latest --changed-surface login-form
```

The release command reads an `allie.evidence.v0` packet and writes a release
summary, a GitHub Checks-style payload, and an HTML decision report:

```sh
.allie/releases/latest/release-summary.json
.allie/releases/latest/github-check.json
.allie/releases/latest/release-report.html
```

It blocks on packet failures, missing evidence for changed surfaces, expired
waivers on touched surfaces, and invalid waiver metadata. Stale evidence,
model-only findings, `needs_review` obligations, and `not_tested` obligations
produce a neutral review-required decision instead of a hard block.

## Autonomous Workbench Loop

```sh
cargo run --locked -- workbench start --manifest examples/autonomous-workbench.yml --out .allie/jobs/autonomous
```

The workbench command is the durable operator entrypoint for autonomous local
work. It writes a job ledger, lifecycle events, step receipts, generated
artifacts, and final pointers under one fresh directory:

```sh
.allie/jobs/autonomous/job.json
.allie/jobs/autonomous/events.jsonl
.allie/jobs/autonomous/steps/discovery/discovery.json
.allie/jobs/autonomous/steps/map/product-map.json
.allie/jobs/autonomous/steps/run/evidence.json
.allie/jobs/autonomous/steps/report/compliance-report.json
.allie/jobs/autonomous/steps/release/release-summary.json
```

Inspect, cancel, or resume the job with:

```sh
cargo run --locked -- workbench status --job .allie/jobs/autonomous
cargo run --locked -- workbench cancel --job .allie/jobs/autonomous
cargo run --locked -- workbench resume --job .allie/jobs/autonomous
```

`workbench start` refuses an existing durable job directory; use `workbench
resume` for an existing job or choose a new `--out` path. Workbench jobs are
local-runner only in this version; `allie map --agent opencode|omp` remains
available as a one-shot advisory mapper until durable session adapters exist.

The one-shot task primitives remain available for debugging or custom
orchestration:

```sh
cargo run --locked -- discover --manifest examples/autonomous-workbench.yml --out .allie/discovery/autonomous
cargo run --locked -- promote-flow --discovery .allie/discovery/autonomous/discovery.json --flow-plan .allie/discovery/autonomous/flow-plan.json --out .allie/discovery/autonomous/generated-flow.yml
cargo run --locked -- run --manifest .allie/discovery/autonomous/generated-flow.yml --out .allie/runs/autonomous
cargo run --locked -- release --packet .allie/runs/autonomous/evidence.json --out .allie/releases/autonomous --changed-surface settings
```

The autonomous loop discovers fixture surfaces, promotes generated flow
candidates into a replayable manifest, captures axe, screenshot, DOM,
accessibility-tree, keyboard, and trace artifacts, and projects the release
decision from that replay evidence. With `model.enabled: true`, the review
step calls the live agentic gateway instead and the manifest must explicitly
declare `model.redaction: none`. That is an honest V0 boundary, not a redaction
claim: unmodified screenshots and video may reach the approved provider, and
the worker records `not_sent` at zero calls or `not_applied` after egress.
Local artifact redaction remains a separate policy. Generated and agentic
claims do not enforce release policy until replayed, scripted, or
human-attested.

## Local Verification

Install the browser worker dependencies once:

```sh
npm install
npx playwright install chromium
```

Then run the repo gates:

```sh
cargo fmt --check
cargo test --locked
npm run worker:smoke
npm run evidence:smoke
npm run auth:smoke
npm run consumer:smoke
npm run release:smoke
npm run autonomous:smoke
cargo run --locked -- run --manifest examples/login-flow.yml --out .allie/runs/latest
cargo run --locked -- release --packet .allie/runs/latest/evidence.json --out .allie/releases/latest --changed-surface login-form
```

The worker smoke proves Playwright plus axe can inspect the checked-in fixture.
The evidence smoke leaves a stable receipt under `.allie/runs/v0-smoke/`. The
auth smoke logs into the local auth fixture, proves the gated route is reached
with no credential value on disk through both `allie run` and `allie verify`,
proves storageState reaches the same gated route, and proves session-less runs
and verifies block instead of auditing the login wall
(`.allie/runs/auth-smoke*` and `.allie/verify/auth-smoke*`). The consumer smoke
proves `allie init` and `allie verify` produce JSON, HTML,
Markdown, JUnit, and SARIF reporters from the same local manifest contract. The
release smoke projects that packet into `.allie/releases/v0-smoke/`. The final
two commands are the V0 live oracle and release projection, leaving inspectable
evidence under `.allie/runs/latest/` and `.allie/releases/latest/`.
The autonomous smoke leaves discovery, generated-flow, richer evidence, and
blocked-release receipts under `.allie/*/autonomous-smoke/`. It also leaves
durable workbench lifecycle receipts under `.allie/jobs/autonomous-smoke/` and
proves the model-enabled workbench path under
`.allie/jobs/autonomous-agentic-smoke/`.

For a cold-start verification path, see [docs/verification.md](docs/verification.md).
