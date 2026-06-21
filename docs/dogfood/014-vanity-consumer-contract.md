# Vanity Consumer Contract Dogfood

Date: 2026-06-20

Command:

```sh
cargo run --locked -- init --manifest .allie/consumer-contract-smoke/vanity/manifest.yml --app-name Vanity --fixture-dir /Users/phaedrus/Development/vanity --force
cargo run --locked -- verify --manifest .allie/consumer-contract-smoke/vanity/manifest.yml --out .allie/consumer-contract-smoke/vanity --project-root /Users/phaedrus/Development/vanity
```

Result: `allie verify` completed the full consumer pipeline and exited `1`
because the release projection was `blocked` by deterministic evidence.

Generated evidence:

- Summary: `.allie/consumer-contract-smoke/vanity/reporters/allie-report.json`
- Verify HTML: `.allie/consumer-contract-smoke/vanity/reporters/allie-report.html`
- Verify Markdown: `.allie/consumer-contract-smoke/vanity/reporters/allie-report.md`
- Product map: `.allie/consumer-contract-smoke/vanity/map/product-map.json`
- Surface map: `.allie/consumer-contract-smoke/vanity/map/surface-map.html`
- Evidence packet: `.allie/consumer-contract-smoke/vanity/run/evidence.json`
- WCAG report: `.allie/consumer-contract-smoke/vanity/report/compliance-report.json`
- WCAG HTML: `.allie/consumer-contract-smoke/vanity/report/compliance-report.html`
- JUnit: `.allie/consumer-contract-smoke/vanity/reporters/junit.xml`
- SARIF: `.allie/consumer-contract-smoke/vanity/reporters/allie.sarif`

Observed summary:

- `reporters.json`: `reporters/allie-report.json` with schema
  `allie.verify.v0`.
- `reporters.wcag_json`: `reporters/allie-compliance-report.json` with schema
  `allie.compliance-report.v0`.
- Discovered/generated surfaces: 15.
- Release status: `blocked`.
- Deterministic failures: 32.
- Infrastructure failures: 0.
- Missing required evidence: 0.
- WCAG report summary: 8 pass, 2 fail, 44 needs review, 1 not tested.

Interpretation: the host-agnostic consumer contract works against a real local
app checkout and preserves a nonzero exit when evidence should block. The
blocked result is product evidence, not a tool failure. The fixture smoke keeps
this `vanity/` receipt directory intact while refreshing its own fixture
artifacts.
