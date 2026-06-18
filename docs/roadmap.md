# Roadmap

## Now

1. Define a formal `allie.evidence.v0` JSON Schema.
2. Define a flow manifest format.
3. Implement `allie run --manifest <path> --out <dir>`.
4. Build a minimal Playwright/axe worker contract.
5. Generate local evidence packets and HTML reports.

## Next

1. Add standards profile mapping for `wcag22-aa`.
2. Add deterministic PR/CI exit semantics.
3. Add screenshot, DOM, and accessibility tree artifact capture.
4. Add model-gateway policy types, but keep provider calls disabled by default.
5. Add fixture app and golden evidence tests.

## Later

1. OpenRouter-backed multimodal first-pass review.
2. GitHub Checks integration.
3. Hosted evidence viewer.
4. SME review workbench.
5. Remediation PR drafting.
6. Browser extension capture companion.
7. Multi-repo dashboard and trends.

## First Acceptance Slice

The first slice is not complete until this command works against a checked-in fixture:

```sh
allie run --manifest examples/login-flow.yml --out .allie/runs/latest
```

Required evidence:

- JSON packet;
- HTML report;
- Playwright route state;
- axe results;
- at least one screenshot;
- deterministic exit code;
- replay instructions.
