# Vanity Static Dogfood

Date: 2026-07-01
Ticket: 025 (child 4 — refresh the clean Vanity static/public run)

## Target

This is the cheapest slice of the dogfood ladder and the static/public class the
epic requires. Vanity (`phaedrus.io`) is a single-page, no-build static site: one
`index.html` on the `@misty-step/aesthetic` kit with a color-mode toggle and an
auto-typing quote colophon, plus two JSON `api/` endpoints that the static
preview does not execute. It has no login wall, so this run exercises the pure
public-content path with no auth setup — the counterpoint to the authenticated
Olympus control-plane slice.

- Target checkout used for this run: `/Users/phaedrus/Development/vanity`
- Target commit: `68a15d080078f28733d142f90dbef47f60b131d2`
- Target status before run: `## master...origin/master` (clean; empty `git status
  --porcelain=v1 --untracked-files=all`, SHA-256
  `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`)
- Target status after run: identical — same empty-porcelain SHA-256, same HEAD.
  Allie audits; it never mutates the target (a hard VISION invariant). Verified
  across two consecutive runs.
- Static server: `python3 -m http.server 4174 --bind 127.0.0.1 --directory
  /Users/phaedrus/Development/vanity`
- Local base URL: `http://127.0.0.1:4174` (pinned in the manifest)
- Auth method: none (public static content)
- Model: agentic vision pass **enabled** (`google/gemini-3.5-flash`, `low`
  effort, `openrouter`, `OPENROUTER_API_KEY` by env name only), matching Vanity's
  own canonical CI manifest. Vanity is public static content, so the model pass is
  safe here — unlike Olympus, whose control-plane data forces `model.enabled:
  false` (`zdr_required`). See the cross-target summary for why the model policy
  differs by target class.

The committed manifest `docs/dogfood/vanity/025-vanity-home.yml` is a pinned
snapshot of Vanity's canonical `.allie/manifest.yml` at the target commit, copied
into this repo so the receipt replays without depending on the target working
tree.

## Commands

Replay variables:

```sh
ALLIE_ROOT=/path/to/allie
VANITY_ROOT=/path/to/vanity          # clean checkout at commit 68a15d0
export ALLIE_BROWSER_WORKER="${ALLIE_ROOT}/workers/browser/run.mjs"
export ALLIE_AGENTIC_WORKER="${ALLIE_ROOT}/workers/agentic/review.mjs"
export OPENROUTER_API_KEY=<your-openrouter-key>   # by env name only
```

Serve the static site (mirrors Vanity's own `allie.yml` CI workflow):

```sh
python3 -m http.server 4174 --bind 127.0.0.1 --directory "${VANITY_ROOT}"
```

Run the verify from the Allie repo root:

```sh
cargo run --locked -- verify \
  --manifest docs/dogfood/vanity/025-vanity-home.yml \
  --out .allie/dogfood/vanity \
  --project-root "${VANITY_ROOT}"
```

`allie verify` exited `1` (`status: blocked`) because the release projection was
blocked by deterministic evidence, not by an Allie failure.

## Evidence

Generated receipt paths (under Allie's ignored `.allie/dogfood/vanity/`):

- Summary JSON: `reporters/allie-report.json`
- Summary Markdown: `reporters/allie-report.md`
- Verify HTML: `reporters/allie-report.html`
- WCAG report JSON: `reporters/allie-compliance-report.json`
- JUnit: `reporters/junit.xml`
- SARIF: `reporters/allie.sarif`
- WCAG report HTML: `report/compliance-report.html`
- Product map: `map/product-map.json`
- Surface map: `map/surface-map.html`
- Evidence packet: `run/evidence.json`
- Browser evidence report: `run/report.html`
- Discovery packet: `discovery/discovery.json`
- Generated flow: `flow/generated-flow.yml`
- Release summary: `release/release-summary.json`
- GitHub check projection: `release/github-check.json`
- Worker + agentic request/response: `run/worker-request.json`,
  `run/worker-response.json`, `run/agentic-request.json`,
  `run/agentic-response.json`

All five committed reporters (JSON / HTML / Markdown / JUnit / SARIF) plus the
WCAG JSON emitted. 48 artifact files total.

Verify facts (canonical run):

- Verify status: `blocked`
- Run status: `fail`
- Evidence exit code: `1`
- Failure class: `blocking-finding`
- Surfaces discovered: `1` (`home`)
- Routes visited: `/` (HTTP `200`)
- States captured: `1` (`home`)
- Workflows exercised: `1` (`autonomous-discovered-flow`)
- Infrastructure failures: `0`
- Scripted failures: `0`
- Deterministic failures: `2`
- State errors: `0`
- Network errors: `0`
- Console errors: `1` — one 404 for a resource the static preview does not
  serve (the `api/` endpoints are not executed by `python3 -m http.server`). This
  is target-environment noise, captured honestly; it is not an Allie failure and
  does not affect the WCAG matrix.
- Keyboard focus order captured: mode toggle → misty step → github → email → body
- Agentic review: 30 criteria, 5 model calls, status `ok`, 0 blocking model
  findings (advisory only)
- Release status: `blocked`

WCAG matrix (canonical run, 55 success criteria):

- `pass`: 39
- `fail`: 1
- `needs_review`: 2
- `not_applicable`: 13
- `not_tested`: 0

### Infrastructure failures vs. real target findings

The blocked release is entirely target evidence. Infrastructure failures: `0`;
missing required evidence: `0`; auth was not in scope. The three findings are
real Vanity accessibility issues, already on record as Vanity backlog items
`008-a11y-kit-contrast` and `009-colophon-pause-control` (Vanity PR #121):

| Criterion | Class | Status | Gating | Detail |
|---|---|---|---|---|
| `1.4.3` Contrast (Minimum) | deterministic (axe-core) | fail | **blocks** | 2 findings (`home-axe-color-contrast-1/2`), serious; the `#q-attr` quote attribution at 2.45:1 |
| `1.4.11` Non-text Contrast | agentic (advisory) | needs_review | no | mode-toggle + link icons below 3:1 |
| `2.2.2` Pause, Stop, Hide | agentic (advisory) | needs_review | no | auto-typing colophon with no in-page pause control |

Only the deterministic `1.4.3` failure gates the release. The two agentic
verdicts are advisory and non-blocking, exactly as the VISION requires: model
judgment informs and prioritizes but never blocks a release on its own until
promoted by scripted reproduction or human attestation.

## Interpretation

Allie produced a full-pipeline, replayable evidence packet — every stage present:
discovery, product/surface map, generated flow, browser evidence, WCAG drilldown,
release projection, JUnit, and SARIF — for a real public static site, on the same
host-agnostic `allie verify` command the target's own CI uses. (Packet stages are
complete; coverage is one surface / one state, as the static site is a single
page — see Residual Risk.) It reached the homepage at HTTP 200 with
zero infrastructure failures, zero state errors, and zero network errors, and
reproduced all three known Vanity findings, correctly splitting the one
deterministic blocker from the two advisory agentic verdicts.

**Determinism check.** The run was executed twice (once against a stale out-dir,
once clean). The deterministic gate was byte-stable across both: deterministic
failures `2`, WCAG failing criteria `1` (`1.4.3`), `not_applicable` `13`,
`not_tested` `0`, `infrastructure_failures` `0`, and `release_status: blocked` —
identical. The advisory agentic layer drifted (`pass` 37↔39, `needs_review` 4↔2,
`ai_pass` 26↔28) as model verdicts moved between runs. This is the honest-
uncertainty invariant working as designed: the release decision rides only on the
stable deterministic spine, while the model layer stays advisory precisely because
it varies. It is also why the report's headline pass/needs-review counts are not,
by themselves, a safe cross-run diff signal (a defect flagged for epic 027).

**Comparison with `docs/dogfood/014-vanity-consumer-contract.md` (2026-06-20).**
The 014 receipt ran the consumer-contract path via `init --fixture-dir`, which
synthesized **15** candidate surfaces from the filesystem and reported **32**
deterministic failures. This 025 run uses the pinned canonical manifest with one
real served state and reports **1** surface / **2** deterministic failures. Both
runs blocked on deterministic evidence with zero infrastructure failures, so the
host-agnostic contract still holds; the surface/finding-count gap is a
mapping-mode difference — fixture-dir discovery over-generates surfaces that do
not correspond to real served routes, which is captured as a mapping-quality
candidate for epic 026 in the cross-target summary.

## Residual Risk

- The agentic verdict layer is nondeterministic run-to-run (observed:
  `needs_review` 4↔2). The manifest declares `known_nondeterminism: []`, which
  under-states this; the deterministic gate is unaffected, but the declaration is
  a manifest-accuracy candidate noted in the cross-target summary.
- `allie verify` does not clean or namespace its `--out` directory. The first run
  in this session inherited stale artifacts from a June-19 run — including an
  obsolete `remediation/` stage the tool has since removed — which could mislead a
  reader inspecting the packet directory. The canonical numbers above are from a
  clean out-dir; this hygiene gap is flagged in the cross-target summary. (The
  current tool emits **no** `remediation/` output; the no-remediation VISION
  invariant holds.)
- The single 404 console error is target-environment noise (unserved `api/`
  endpoint under the static preview), not an Allie or accessibility finding.
- The finer-grained 60-obligation coverage ledger
  (`run/evidence.json .coverage.standards_obligations_evaluated`) lists 4
  obligations as not-tested (reflow, zoom-reflow, keyboard-traversal,
  reduced-motion) that roll up into passing criteria at the 55-success-criterion
  headline (`not_tested: 0`). Flagged for epic 027.
- This is evidence visibility only, not a legal compliance claim.
- All generated `.allie/dogfood/vanity/` artifacts are local ignored state and
  should be regenerated (into a clean out-dir) when reviewing this receipt.
