---
colors:
  ink: "#111111"
  paper: "#F6EFE1"
  panel: "#FFF9EA"
  line: "#111111"
  primary: "#0A5EA8"
  blue: "#0A5EA8"
  accent: "#FFD22E"
  pass: "#167A3A"
  fail: "#C21D1D"
  review: "#FFD22E"
  muted: "#4C4A42"
  white: "#FFFFFF"
  black: "#000000"
  body: "#25231F"
  faint: "#D8CFBD"
  failWash: "#FFF1E8"
  reviewWash: "#FFF6C9"
  passWash: "#EDFFF3"
  evidenceWash: "#FBF4DF"
  neutralWash: "#EEE4CE"
  reviewLight: "#FFFBE2"
typography:
  fontFamily: "ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
  monoFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace"
rounded:
  none: "0px"
spacing: ["4px", "8px", "12px", "16px", "24px", "32px"]
---

# Overview

Allie's durable visual baseline is **clean-atomic comic-ops**: hard square
panels, ink-first ledgers, blue section tabs, yellow review markers, and
evidence proof strips. The aesthetic comes from the user-provided Allie concept
board and the reachable Misty Step aesthetic CSS at
`https://cdn.jsdelivr.net/gh/misty-step/aesthetic@v2.5.1/aesthetic.css`.

The npm package name `@misty-step/aesthetic` was not available from the public
npm registry during adoption, and the requested commit URL
`https://cdn.jsdelivr.net/gh/misty-step/aesthetic@9bbe0f9/aesthetic.css`
returned 404. Allie therefore embeds a local, offline-safe token subset in
generated reports instead of depending on a remote stylesheet at report-view
time.

## Colors

- `ink`, `line`: primary text and structural borders.
- `paper`, `panel`: warm evidence-report background and panels.
- `blue`: section tabs and navigational emphasis.
- `accent`: review-required and proof-strip emphasis.
- `pass`, `fail`, `review`: status slabs only where fast scanning matters.

Accessibility evidence clarity wins over decoration. Status color must always
pair with text, never stand alone.

## Typography

Allie uses system sans for report readability and system monospace for evidence
ids, commands, counts, and packet references. Labels are uppercase and compact;
body copy stays plain and legible.

## Layout

Evidence reports use dense, scannable ledgers. Panels are square, bordered, and
flat. Avoid soft cards, soft shadows, glass effects, gradient text, decorative
orbs, and ambient motion.

## Shapes

Radius is `0px` for durable report surfaces. Controls, chips, proof strips,
artifact frames, tables, and status panels should read as hard-edged audit
material, not consumer SaaS cards.

## Components

- **Section tab:** blue background, white uppercase label, hard ink border.
- **Proof strip:** a horizontal run of counts or artifacts that proves the
  current claim.
- **Ledger:** table or matrix with hard grid lines and tabular numbers.
- **Status slab:** pass/fail/review chip with text and high-contrast fill.
- **Evidence frame:** screenshot/video thumbnail in a hard bordered panel.

## Do's and Don'ts

- Do keep WCAG legibility, keyboard focus, and contrast above style fidelity.
- Do keep report copy about evidence, status, confidence, and residual review.
- Do not promise legal compliance or remediation.
- Do not copy the concept board composition wholesale; treat it as direction.
- Do not add decorative texture where it competes with audit evidence.
