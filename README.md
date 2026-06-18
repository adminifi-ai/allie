# Allie

Allie is a Rust-first accessibility evidence harness and release intelligence system.

The product goal is not to be another accessibility scanner. Allie should map staged web applications and critical user flows, run deterministic accessibility checks, capture replayable evidence, enrich judgment-heavy criteria with multimodal agents, and make accessibility status visible in pull requests and release decisions.

Positioning:

> Accessibility evidence for every release.

## Why This Exists

Accessibility work is often split across manual expert review, browser extensions, point-in-time axe runs, ad hoc screenshots, and release conversations that are hard to reproduce. Allie turns that into an evidence system:

- deterministic checks where machines can be certain;
- scripted browser flows where interaction behavior matters;
- screenshots, video, DOM, and accessibility tree artifacts where human or agent review needs context;
- standards-mapped status across WCAG, ADA, Section 508, and client policy packs;
- PR and release gates that block real regressions without pretending uncertain findings are certain.

## Initial Shape

- Rust CLI and orchestrator.
- Node/Playwright/axe worker boundary for browser automation.
- Evidence packets as the durable contract.
- Model gateway for multimodal first-pass review, with OpenRouter only behind strict privacy and provider-routing policy.
- Local HTML/JSON reports first; hosted dashboards later.

## Repository Map

- [SPEC.md](SPEC.md): product contract and acceptance model.
- [docs/architecture.md](docs/architecture.md): proposed system design.
- [docs/evidence-contract.md](docs/evidence-contract.md): first evidence schema shape.
- [docs/naming.md](docs/naming.md): naming decisions and alternates.
- [docs/roadmap.md](docs/roadmap.md): proposed build sequence.

## Current CLI Placeholder

```sh
cargo run --locked
```

The binary currently prints the project line and next target. The first real CLI milestone is:

```sh
allie run --manifest <flow.yml>
```

That command should execute one authenticated journey against one staged app, run Playwright plus axe, and emit a replayable evidence packet.
