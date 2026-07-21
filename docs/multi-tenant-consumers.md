# Multi-Tenant & Regulated Consumers — Design Considerations

Status: **Considerations captured 2026-06-22. NOT committed design.**
Allie's stated direction is to be a host-agnostic, *multiply-deployable* WCAG
evidence tool rather than a bespoke per-project solution. Engagement with
prospective consumers — in particular multi-tenant platforms operating under a
regulatory accessibility deadline — surfaces design pressures Allie does not yet
model as first-class. This note captures those pressures as inputs and open
questions. How (or whether) to absorb them needs more research and design.

## The shape of the consumer this anticipates

A common, demanding deployment target:

- **Multi-tenant**: a single codebase rendered differently per client via
  configuration; the same page may render in many distinct configurations.
- **Client-controlled presentation**: clients manage their own themes (sometimes
  dynamic) and author content through WYSIWYG editors. The client's choices can
  be non-conformant while the *platform* remains legally accountable.
- **Production-faithful test targets**: meaningful testing requires real config,
  not local seed data. Per-client "baseline" environments that mirror production
  are the natural target surface.
- **Regulatory deadline + a compliance team**: the primary consumer of the
  output is an internal accessibility/compliance team working against a legal
  deadline, not only developers. The report is an *audit artifact*, and "never
  auto-greenlight" is a hard requirement.
- **Heterogeneous CI/hosting**: not always GitHub; e.g. Azure DevOps pipelines,
  manual/FTP deploys, mixed cloud. Allie must plug into whatever exists.
- **Brand-wide AI governance**: allowed model providers and data-retention rules
  are set centrally and may restrict which providers/models are permissible.

## Design pressures (open questions, not decisions)

1. **Tenant / configuration as a coverage axis.** Allie models
   `surface × state`. Multi-tenant adds `config/tenant × surface × state ×
   workflow`. Open questions: how the manifest expresses *a set* of targets (one
   per client/baseline) rather than a single `base_url`; how the coverage matrix
   and report roll up per-config and across-configs; how to keep the matrix from
   exploding (template-clustering + WCAG-EM sampling + cross-config dedup — many
   configs render a given page identically, so judge the visually-distinct ones
   once). See the multi-page section of `criteria-assessability-research.md`.

2. **Finding ownership / action attribution.** A failure caused by a
   client's theme or authored content is the client's to fix (often via a
   contractual conversation, not a code change), but the platform is
   accountable. A new classification — attributing each finding to a layer
   (**platform code vs client theme vs client content**) and likely action owner
   — would be high-value audit context and is not modeled today.

3. **Provider allowlist and supported ZDR routing are enforced; upstream
   retention remains an external attestation.** The gateway fails closed unless
   the selected provider is allowlisted and its resolved endpoint matches the
   canonical preset. When `zdr_required` is true, the OpenRouter adapter sends
   `provider.zdr: true`, disables fallbacks, explicitly declines OpenRouter
   response caching with `X-OpenRouter-Cache: false`, and requests router
   metadata; providers without a declared ZDR adapter fail before the worker is
   spawned. Per-attempt receipts record the requested policy and actual routed
   provider/model. They prove what Allie requested and observed,
   not the provider's internal retention behavior. The agentic model remains a
   *per-consumer* choice, reinforcing that model selection belongs in the
   manifest rather than a universal default.

4. **Run modes: changed-scope vs full sweep.** A regression-prevention mode that
   scans only the surfaces affected by a PR's changes, plus a periodic full
   sweep, is a likely requirement. The report should distinguish **new
   regressions introduced by a change** from **pre-existing site-wide debt**.
   (`release --changed-surface` is a partial foundation.) Note the common failure
   mode of periodic-only jobs: nobody reads them — so PR-time signal on *new*
   code matters.

5. **Actionability split.** Findings naturally divide into "low-hanging fruit"
   (e.g. a missing `alt` attribute) and findings that require a product/design
   decision (theme, contrast, layout). Allie should surface that split as audit
   context only; any fix workflow belongs in a downstream product consuming the
   evidence packet.

6. **Host bridges beyond GitHub.** An Azure DevOps bridge sits alongside the
   GitHub Actions one — `docs/ci/azure-pipelines-allie-verify.yml` already
   exists as a starting point and should be kept current as the consumer
   contract evolves.

7. **Report-as-audit-artifact.** When the primary reader is a compliance team,
   the standalone report (provenance, per-criterion evidence, confidence,
   ownership) matters more than the CI pass/fail. The existing "evidence
   visibility, not a legal compliance guarantee" framing is the right posture.

## How this intersects with the assessability research

The multi-tenant fan-out makes several threads in
`criteria-assessability-research.md` load-bearing rather than optional:
template-clustering/sampling (to tame the configuration matrix), the
deterministic-probe push (a team on a deadline wants the maximum number of
criteria committed at high confidence, shrinking the manual surface), and the
confidence flywheel (a compliance team is exactly the human-in-the-loop whose
feedback reduces false positives over time).

## Not yet decided

Whether Allie absorbs these as first-class concepts now (while the architecture
is young) or defers them is an open product decision. The risk to weigh: a
bespoke per-consumer fork is the explicit anti-goal, so requirements like the
tenant axis and provider-allowlist enforcement are candidates to design in
generically rather than retrofit.
