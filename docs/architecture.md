# Architecture

## Recommended Shape

Allie should be a Rust orchestrator with narrow worker adapters.

```text
allie CLI
  -> config and flow manifest parser
  -> run planner and budget policy
  -> Playwright/axe worker adapter
  -> evidence packet writer
  -> standards ledger
  -> model gateway
  -> report and PR adapters
```

## Module Boundaries

### Rust Core

Rust owns:

- CLI surface;
- flow manifest schema;
- run planning;
- policy and budgets;
- evidence packet schema and hashing;
- standards profile mapping;
- ledger indexing;
- report generation;
- PR/check payload generation;
- model gateway policy enforcement.

### Browser Worker

Node owns:

- Playwright browser execution;
- page setup and authenticated state;
- route and interaction steps;
- axe scans through `@axe-core/playwright`;
- screenshot, video, trace, DOM, and accessibility tree capture.

Rust should treat the worker as an external process with a typed request/response contract. Do not let browser automation details leak through the whole codebase.

### Model Gateway

The model gateway owns:

- provider allowlists;
- ZDR requirements;
- spend and call budgets;
- redaction policy;
- prompt templates;
- model metadata capture;
- response validation;
- no-fallback enforcement.

OpenRouter is an implementation detail behind this gateway, not a dependency that should spread through product logic.

### Evidence Store

V0 can write local files.

Later versions can add:

- SQLite for local indexed runs;
- object storage for large artifacts;
- Postgres for hosted multi-tenant dashboards;
- signed URLs for report sharing.

## Form Factors

Build in this order:

1. Rust CLI.
2. Playwright/axe worker.
3. Local HTML/JSON report.
4. GitHub check / CI adapter.
5. Hosted evidence viewer.
6. Dashboard and trend ledger.
7. SME review workbench.
8. Remediation PR drafting.
9. Browser extension capture companion.

## Design Constraint

The interface should stay deep and narrow:

- one manifest in;
- one evidence packet out;
- deterministic exit semantics;
- optional model enrichment behind explicit policy.

Avoid a semantic workflow engine. Allie should orchestrate evidence collection and release decisions, not become a general-purpose agent platform.
