# Architecture

`VISION.md` owns the product boundaries. This document describes the intended
technical shape; when it conflicts with the vision, update this document rather
than preserving a second direction.

## Recommended Shape

Allie should be a Rust orchestrator with narrow worker adapters.

```text
allie CLI
  -> repository/context discovery + optional manifest overrides
  -> run planner and budget policy
  -> isolated target/environment adapter
  -> durable accessibility test plan
  -> Playwright/axe worker adapter
  -> evidence packet writer
  -> standards ledger
  -> model gateway
  -> report bundle + publisher adapters
```

## Module Boundaries

### Rust Core

Rust owns:

- CLI surface;
- discovered-context and optional flow-manifest schemas;
- run planning;
- policy and budgets;
- accessibility test-plan schema and promotion state;
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

### Target and Environment Boundary

Allie receives a pinned, read-only checkout and, when available, a runnable
application through narrow adapters. Builds, target processes, generated tests,
and exploration execute inside a declared ephemeral sandbox. Production-like
data remains governed by the consuming organization's sanitization policy; an
environment adapter returns the endpoint, revision, configuration inventory,
and sanitization attestation that Allie records.

Generated executable tests are run from an Allie-owned workspace and discarded.
The durable asset is the compact accessibility test plan: surfaces, states,
variants, obligations, methods, and required evidence. A proposed permanent test
patch may be published as an artifact, but the audit never applies it.

### Evidence Store and Publishers

V0 can write local files.

Later versions can add:

- SQLite for local indexed runs;
- object storage for large artifacts;
- signed URLs for report sharing.

Storage and publication are separate adapter boundaries. Local disk, GitHub,
Azure, object storage, or a future system receive the same canonical packet and
report bundle; host-specific adapters do not own accessibility policy.

## Form Factors

Build in this order:

1. Rust CLI.
2. Playwright/axe worker.
3. Local HTML/JSON report.
4. Zero-config repository discovery with optional manifest enrichment.
5. Durable test-plan compiler with ephemeral generated tests.
6. Sandboxed target/environment adapters.
7. GitHub check / CI publisher, followed by host-neutral publishers.
8. Optional ingestion of externally authored human-review packets.

A hosted viewer, dashboard, interactive SME workbench, or browser extension is
not a near-term core form factor. Such products may consume Allie's portable
packets later without turning the contained actor into a service or interactive
agent.

## Design Constraint

The interface should stay deep and narrow:

- repository access in, with optional context and policy overrides;
- one evidence packet out;
- one progressively disclosed report bundle over that packet;
- deterministic exit semantics;
- optional model enrichment behind explicit policy.

Avoid a semantic workflow engine. Allie should orchestrate evidence collection and release decisions, not become a general-purpose agent platform.
