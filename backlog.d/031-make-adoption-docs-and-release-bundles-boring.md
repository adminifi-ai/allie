# Make adoption docs and release bundles boring

Priority: P2 · Status: pending · Estimate: XL

## Goal
Make Allie easy to adopt from a fresh checkout or release bundle by proving install, doctor, verify, docs, and distribution paths end to end.

## Oracle
- [ ] A five-minute quickstart gets a user from install to first report with one fixture and one app manifest.
- [ ] Troubleshooting docs cover browser worker setup, auth/storageState, model keys, artifact paths, release interpretation, and common false blockers.
- [ ] The release workflow validates a downloaded bundle after upload, not just the local build artifact.
- [ ] Release artifacts cover the intended platforms or explicitly document the current platform limit.
- [ ] Canonical docs separate current product contracts from historical plan artifacts that may mention superseded scope.

## Verification System
- Claim: A developer can install Allie, run `allie doctor`, generate a report, and understand the result without reading the whole repository or relying on chat context.
- Falsifier: The README names a release path that does not exist, the bundle cannot run its own smoke after download, quickstart steps miss dependencies, or stale docs contradict the no-remediation vision.
- Driver: Fresh-directory quickstart test, release bundle smoke, docs link check, and a historical-doc drift audit.
- Grader: New-user walkthrough receipt, post-upload bundle validation, docs checklist, and no unresolved canonical-doc contradictions.
- Evidence packet: `.allie/adoption-smoke/`, release workflow logs, and docs receipts.
- Cadence: Before first public tag, after release workflow changes, and after product-scope changes.

## Children
1. Add a short quickstart that uses the release bundle or local binary and produces a report from a checked-in fixture.
2. Add `docs/troubleshooting.md` and a glossary for accessibility terms Allie uses in reports.
3. Add a docs index that marks canonical product docs versus historical plans and receipts.
4. Audit historical docs for remediation language and either mark them historical or update canonical links so the current no-remediation vision is not contradicted.
5. Add release-bundle post-upload validation and document the first supported platform matrix.
6. Add a fresh-directory adoption smoke that exercises `allie init`, `allie doctor`, and `allie verify`.
7. Record the first public release checklist and residual gaps before tagging.

## Notes
- `README.md` already describes release install paths, while the repository currently has no public tags or GitHub releases.
- This epic is adoption and distribution polish, not hosted SaaS work.
- All docs must preserve the boundary: Allie audits, maps, reports, and hands off evidence; it does not remediate.
