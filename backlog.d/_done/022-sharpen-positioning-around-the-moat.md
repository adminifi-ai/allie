# Sharpen positioning around the real moat

Priority: P2 · Status: done · Estimate: S

## Goal
Docs and positioning lead with Allie's true differentiation — replayable
evidence packet + complete WCAG obligation ledger tied to a code revision —
contrast against overlays, and resolve scope edges honestly.

## Oracle
- [x] README/positioning lead with the evidence-packet + obligation-ledger + replay-to-revision moat, not "AI".
- [x] A "why not an overlay" + "no legal-compliance promise" trust contrast exists (cite FTC v. AccessiBe, $1M, Apr 2025).
- [x] Mobile-web coverage is real: WCAG 2.2 mobile-relevant criteria (1.4.10 reflow, 1.3.4 orientation, 2.5.1 pointer gestures, 2.5.4 motion actuation, 2.5.8 target size) are audited at mobile viewports — these are WCAG requirements, so in scope, not a non-goal. Only native (non-web) mobile apps get an explicit scope statement in VISION.md.
- [x] The 2.2-AA ledger cleanly degrades to a 2.1-AA view for EAA consumers (EAA's harmonized baseline is still WCAG 2.1 AA).

## Receipts
- `npm run verify` passed.
- `npm run evidence:smoke` verifies 390x844 mobile-web metadata plus mobile axe/screenshot artifacts.
- `npm run coverage:smoke` verifies the `wcag21-aa` profile view with a 50-criterion denominator, WCAG 2.2-only exclusions, and explicit `wcag21-aa:4.1.1-parsing` legacy gap.
- Thermo review fixed the report gallery implementation before closeout.

## Children
1. Reframe README/positioning around the ledger + provenance moat.
2. Add the "why not an overlay" / no-legal-promise contrast.
3. Cover WCAG mobile-web criteria at mobile viewports; state native-app (non-web) scope explicitly in VISION.
4. 2.2-AA → 2.1-AA ledger view for EAA.

## Notes
**Why:** competitive lane. Autonomous discovery + vision are being commoditized by the free "MCP + axe-core + LLM" stack (Deque axe MCP Server, Playwright MCP) — so lead with what no competitor gives you: a local-first, no-score, no-silent-gaps audit artifact you fully own and can replay to a revision. The market is actively burned by AI-a11y overclaiming (FTC/AccessiBe) — Allie's evidence-and-gaps honesty is a trust moat worth naming. Mobile-web behavior is required by WCAG, so Allie must cover it at mobile viewports; native mobile apps (iOS/Android, non-web) are the only genuine scope question — state that boundary explicitly rather than leaving a silent gap.
