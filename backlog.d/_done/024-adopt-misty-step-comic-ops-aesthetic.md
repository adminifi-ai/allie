# Adopt the Misty Step comic-ops aesthetic baseline

Priority: P2 · Status: done · Estimate: M

## Goal
Evaluate and adopt the clean-atomic comic-ops flavor for Allie's evidence
reports, local HTML output, and release-decision surfaces.

## Oracle
- [x] `DESIGN.md` or an equivalent design section names Allie's chosen flavor,
      likely `clean-atomic`, and the allowed deviations for accessibility
      evidence reports.
- [x] At least one representative report surface is rendered or mocked with
      comic-ops tokens, proof strips, ledgers, and hard square panels.
- [x] The implementation uses `@misty-step/aesthetic` commit `9bbe0f9` or
      later, or records a deliberate no-adoption decision.
- [x] Allie keeps evidence clarity and WCAG legibility ahead of decorative
      texture.
- [x] The repo's canonical verification command passes after implementation.

## Notes
Reference board:
`http://serenity.tail5f5eb4.ts.net:8788/allie-clean-atomic-concept.png`.
Generated art is inspiration only; rebuild product copy and UI from tokens.

## Receipts
- `DESIGN.md` names the `clean-atomic comic-ops` baseline, token set, source
  decision, and evidence-legibility boundary.
- `docs/design-contract.md` records the concept-board and Misty Step provenance.
- `.allie/reports/coverage-matrix-smoke/compliance-report.html` renders the
  representative WCAG compliance report with `clean-atomic`,
  `misty-step-v2.5.1-local-subset`, `proof-strip`, `hard-panel`, and `ledger`
  markers.
- Playwright screenshots inspected:
  `/tmp/allie-024-review-desktop.png` and `/tmp/allie-024-review-mobile.png`;
  both fit without horizontal overflow.
- `npm run verify` passed.
