# Package the host-agnostic consumer contract

Priority: P1 - Status: pending - Estimate: L

## Goal

Make Allie easy to adopt in arbitrary repositories and CI hosts through a
portable local contract first, with GitHub, Azure, and other CI integrations as
thin adapters over the same artifacts.

## Oracle

- [ ] `allie init` can scaffold a minimal `.allie/manifest.yml` and explain the
  next local verification command without assuming GitHub.
- [ ] `allie verify` can run discovery/map/report/release projection for a
  configured app and produce the same JSON/HTML artifacts locally and in CI.
- [ ] Reporter outputs include stable `json`, `html`, `markdown`, `junit`, and
  `sarif` or documented reasons for omission.
- [ ] GitHub and Azure examples call the same CLI contract and do not fork
  policy logic.
- [ ] A dogfood run proves the same manifest works in at least one real app repo
  and one local fixture.

## Verification System

- Claim: consuming apps can use Allie without coupling to a code host, CI vendor,
  or hosted dashboard.
- Falsifier: policy lives in a GitHub workflow, Azure output diverges from local
  output, setup requires manual route/story authoring for the first smoke, or
  reports cannot be consumed outside the CLI.
- Driver: local fixture run plus one dogfood app run using the same scaffolded
  manifest and reporter set.
- Grader: artifact parity checks, CI-example diff checks, generated manifest
  schema validation, and final report link checks.
- Evidence packet: `.allie/consumer-contract-smoke/` plus dogfood app receipts.
- Cadence: after the durable job and coverage-matrix epics define the V1 packet
  shape.

## Children

1. Add `allie init` manifest scaffolding with no host assumptions.
2. Add `allie verify` as the primary operator command over existing primitives.
3. Define the reporter matrix and artifact naming contract.
4. Add GitHub and Azure examples as thin wrappers.
5. Dogfood the contract in Vanity first, then pick Linejam, Sploot, or Misty
   Step for a second app shape.

## Notes

**Why:** The operator UX lane found the repo has strong evidence primitives but
still exposes an operator-heavy command chain. The consumer contract should be a
single portable CLI path whose outputs any CI host can archive or annotate.

