# Competitive Landscape

Last reviewed: 2026-06-30

## Positioning Thesis

Allie should not compete as another accessibility scanner. The durable
differentiator is an evidence-packet workbench: autonomous surface discovery,
generated replayable tests, WCAG criterion-by-surface visibility, artifact
drilldown, agentic context that remains non-blocking until promoted, and release
decisions that can be audited.

Competitors prove the market wants automation, guided manual review, dashboards,
AI assistance, and enterprise audit workflows. Allie should use those lessons
while staying sharper about local reproducibility, privacy policy, provenance,
and host-agnostic CI/release contracts.

The overlay trust gap is now part of the category context. In April 2025, the
FTC approved a final order requiring accessiBe to pay $1 million and barring
unsupported claims that automated products can make or keep websites
WCAG-compliant. Allie should not answer that market scar with stronger AI
claims. It should answer with owned evidence packets, criterion ledgers,
replay-to-revision provenance, and explicit residual review needs.

Each entry records the same fields so roadmap discussions can compare Allie
against scanner, dashboard, manual-audit, browser-cloud, design, and AI-assisted
alternatives without collapsing them into one vague category.

The record shape is machine-checked by `npm run landscape:smoke`. Keep the field
labels stable unless the script changes with the document contract. The script
checks structure, source presence, and freshness; it does not replace human or
agent review of vendor claims.

## Landscape Records

### Deque axe-core

- **Category:** Open-source accessibility rules engine.
- **Automation depth:** Automated Web UI checks that can be embedded in browser,
  unit, integration, and end-to-end tests.
- **Standards claims:** Deque describes axe-core coverage for WCAG 2.0, 2.1,
  and 2.2 at A, AA, and AAA, plus standards that rely on WCAG.
- **CI/PR integration:** Deque describes axe-core as built to integrate with
  existing test environments so teams can automate accessibility testing beside
  regular functional testing.
- **Evidence artifacts and replay:** Produces rule results and WCAG-tagged
  violations, but replay and artifact graph semantics belong to the calling
  harness.
- **Manual review workflow:** None by itself; it is a machine-check engine.
- **AI/agentic claims:** None in the open-source engine.
- **Privacy/governance posture:** Local library execution can stay inside the
  consumer test environment.
- **Allie differentiation:** Treat axe-core as deterministic evidence inside a
  richer packet, not as the whole product surface, story map, or release policy.
- **Sources:** [axe-core GitHub](https://github.com/dequelabs/axe-core),
  [Deque axe-core](https://www.deque.com/axe/axe-core/)

### accessiBe and accessibility overlays

- **Category:** Accessibility overlay/widget vendor.
- **Automation depth:** Automated website modifications and AI-assisted
  accessibility claims around a plug-in overlay.
- **Standards claims:** The FTC alleged accessiBe claimed its plug-in could make
  any website WCAG-compliant; the final order bars unsupported automated
  WCAG-compliance and continuing-compliance representations.
- **CI/PR integration:** Overlay installed into the site experience rather than
  a repo-owned CI evidence loop.
- **Evidence artifacts and replay:** Public trust issue is the opposite of
  Allie's wedge: the FTC action centered on unsupported automated compliance
  claims, not replayable per-revision evidence packets.
- **Manual review workflow:** Not the primary trust contract in the FTC case.
- **AI/agentic claims:** AI-powered accessibility tool claims were central to
  the FTC's January 2025 complaint and April 2025 final order.
- **Privacy/governance posture:** Third-party overlay posture; Allie should keep
  local-first artifacts and provider routing explicit.
- **Allie differentiation:** Lead with evidence, status, confidence, residual
  review needs, and replay commands. Never promise legal compliance or automated
  remediation.
- **Sources:** [FTC final order](https://www.ftc.gov/news-events/news/press-releases/2025/04/ftc-approves-final-order-requiring-accessibe-pay-1-million),
  [FTC accessiBe case page](https://www.ftc.gov/legal-library/browse/cases-proceedings/2223156-accessibe-inc)

### Deque axe DevTools and axe Platform

- **Category:** Enterprise developer, guided testing, manual workflow, and
  monitoring suite.
- **Automation depth:** Automated axe checks plus guided tests, full-page and
  user-flow testing, reporting, and platform workflow.
- **Standards claims:** WCAG-oriented accessibility testing, with Deque warning
  users to avoid false positives and combine automated, semi-automated, and
  manual tests.
- **CI/PR integration:** Developer tooling and integrations such as Jira and
  workflow testing; Sauce Labs also exposes Deque integration material.
- **Evidence artifacts and replay:** Reporting and sharing are platform/tooling
  features; replay is not the public contract in the same packet-first sense.
- **Manual review workflow:** Intelligent Guided Tests ask structured questions
  about the app state and return guided results.
- **AI/agentic claims:** axe DevTools Extension Pro markets AI-enhanced features
  alongside Intelligent Guided Tests.
- **Privacy/governance posture:** Enterprise platform posture; public pages do
  not make local packet provenance the primary artifact.
- **Allie differentiation:** Keep local evidence packets and replay commands as
  the source of truth, with platform adapters as optional projections.
- **Sources:** [axe Platform](https://www.deque.com/axe/),
  [axe DevTools Extension](https://www.deque.com/axe/devtools/extension/),
  [Intelligent Guided Tests docs](https://docs.deque.com/devtools-for-web/4/en/devtools-igt/)

### Evinced

- **Category:** AI-powered accessibility automation platform.
- **Automation depth:** Markets automatic finding, clustering, tracking, and
  prevention of accessibility issues across web and mobile apps.
- **Standards claims:** WCAG-oriented accessibility testing and automation, with
  claims of finding issues that previously required manual audits.
- **CI/PR integration:** Public materials emphasize fitting test automation into
  development workflows.
- **Evidence artifacts and replay:** Findings and tracking are central; public
  positioning does not expose a versioned evidence-packet/replay contract.
- **Manual review workflow:** Automation is positioned as a complement to manual
  audits and accessibility-manager workflows.
- **AI/agentic claims:** Explicit AI-powered detection and clustering claims.
- **Privacy/governance posture:** Public pages do not make model routing,
  redaction receipts, or promotion state the primary contract.
- **Allie differentiation:** Preserve AI output as review context until replayed,
  scripted, or human-attested evidence promotes it.
- **Sources:** [Evinced](https://www.evinced.com/),
  [Evinced technology](https://www.evinced.com/technology),
  [Evinced automation overview](https://www.evinced.com/blog/introduction-to-test-automation-for-accessibility-managers)

### Level Access

- **Category:** Enterprise accessibility platform and services.
- **Automation depth:** Automated scans, expert evaluation, reporting, training,
  and broader accessibility program support.
- **Standards claims:** Public statement names WCAG 2 Level A/AA, Section 508,
  and EN 301 549 support.
- **CI/PR integration:** Enterprise platform and product suite rather than a
  single local CLI contract.
- **Evidence artifacts and replay:** Reporting and evaluation are central;
  replayable local evidence packets are not the public interface.
- **Manual review workflow:** Expert evaluation and user testing are part of the
  service/platform model.
- **AI/agentic claims:** Not the primary public differentiator in the cited
  sources.
- **Privacy/governance posture:** Enterprise accessibility governance posture.
- **Allie differentiation:** Serve engineering teams earlier in the repo and CI
  loop before enterprise program-management artifacts are needed.
- **Sources:** [Level Access](https://www.levelaccess.com/),
  [Level Access accessibility statement](https://www.levelaccess.com/accessibility-statement/),
  [Automated testing guide](https://www.levelaccess.com/blog/automated-accessibility-testing-a-practical-guide-to-wcag-coverage/)

### Accessibility Insights

- **Category:** Free guided assessment and browser-extension workflow.
- **Automation depth:** FastPass automated checks, tab-stop helpers, and guided
  assessment steps.
- **Standards claims:** Assessment workflow targets WCAG 2.1 Level AA.
- **CI/PR integration:** Primarily browser/assessment workflow; axe rules can be
  integrated separately through build tooling.
- **Evidence artifacts and replay:** Assessment and Quick Assess can export JSON
  grouped by success criteria; export is useful evidence, but replay belongs to
  the user workflow.
- **Manual review workflow:** Strong guided manual assessment with pass/fail and
  incomplete requirement states.
- **AI/agentic claims:** No core AI-agent claim in official docs.
- **Privacy/governance posture:** Browser-extension/local assessment posture;
  official FAQ and community notes emphasize local data for extension use.
- **Allie differentiation:** Automate surface discovery and pre-fill review
  context across CI artifacts rather than requiring extension-driven setup.
- **Sources:** [Overview](https://accessibilityinsights.io/docs/web/overview/),
  [Assessment](https://accessibilityinsights.io/docs/web/getstarted/assessment/),
  [FAQ](https://accessibilityinsights.io/docs/web/reference/faq/)

### Pa11y

- **Category:** Open-source CLI, CI, webservice, and dashboard family.
- **Automation depth:** URL/page scanning through CLI and Node API, plus CI and
  dashboard variants.
- **Standards claims:** Accessibility issue reporting through automated checks;
  CI-focused ecosystem commonly targets WCAG-oriented checks.
- **CI/PR integration:** Pa11y CI is explicitly built for continuous integration.
- **Evidence artifacts and replay:** CLI/JSON/webservice outputs and dashboard
  history; no first-class browser-flow artifact graph.
- **Manual review workflow:** Not a guided manual audit workflow.
- **AI/agentic claims:** None in the core project.
- **Privacy/governance posture:** Open-source local/CI operation is possible.
- **Allie differentiation:** Go deeper on authenticated workflows, generated
  Playwright states, standards matrices, and release decisions.
- **Sources:** [Pa11y](https://pa11y.org/),
  [Pa11y GitHub](https://github.com/pa11y/pa11y),
  [Pa11y CI](https://github.com/pa11y/pa11y-ci)

### Lighthouse and Chrome DevTools audits

- **Category:** Developer audit runner and score/report surface.
- **Automation depth:** Audits a URL from DevTools, CLI, Node, or CI; includes
  accessibility audits among performance, SEO, and best-practices checks.
- **Standards claims:** Accessibility score is a weighted average of Lighthouse
  accessibility audits, with weighting based on axe user-impact assessments.
- **CI/PR integration:** Lighthouse CI can prevent regressions on sites.
- **Evidence artifacts and replay:** Generates Lighthouse reports; score and
  audit output are not a criterion-by-surface evidence graph.
- **Manual review workflow:** Helps developers inspect failed audits; not a full
  manual WCAG assessment workflow.
- **AI/agentic claims:** None in official Lighthouse docs.
- **Privacy/governance posture:** Local browser/CLI execution is possible.
- **Allie differentiation:** Avoid global scores; expose exact criteria, product
  states, residual review needs, and release-policy decisions.
- **Sources:** [Lighthouse overview](https://developer.chrome.com/docs/lighthouse/overview),
  [Lighthouse accessibility scoring](https://developer.chrome.com/docs/lighthouse/accessibility/scoring),
  [Lighthouse GitHub](https://github.com/GoogleChrome/lighthouse)

### BrowserStack Accessibility

- **Category:** Browser/device cloud with accessibility testing.
- **Automation depth:** Automated accessibility testing through Automate using
  axe-core plus broader accessibility product features.
- **Standards claims:** Axe-core-backed rule reports and accessibility issue
  detection; BrowserStack markets accessibility testing for WCAG-oriented work.
- **CI/PR integration:** Fits existing BrowserStack Automate/Selenium workflows.
- **Evidence artifacts and replay:** Cloud test session artifacts and axe
  reports are runner outputs; policy and packet semantics remain external.
- **Manual review workflow:** Browser/device cloud can support manual and
  screen-reader testing workflows.
- **AI/agentic claims:** BrowserStack markets an accessibility issue-detection
  AI agent that brings human-like judgment to accessibility testing.
- **Privacy/governance posture:** Hosted cloud execution; governance follows
  BrowserStack account and product controls.
- **Allie differentiation:** Make browser clouds execution adapters while Allie
  owns local policy, evidence provenance, and replay semantics.
- **Sources:** [BrowserStack accessibility testing](https://www.browserstack.com/accessibility-testing),
  [Automate accessibility docs](https://www.browserstack.com/docs/automate/selenium/accessibility-testing),
  [BrowserStack AI agents](https://www.browserstack.com/accessibility-testing/ai-agents)

### Sauce Labs Accessibility

- **Category:** Continuous testing cloud with accessibility integrations.
- **Automation depth:** Sauce integrates Deque/axe into web and mobile testing
  workflows on Sauce infrastructure.
- **Standards claims:** Deque/Sauce pages describe compliance and global
  accessibility standards in the context of axe testing.
- **CI/PR integration:** Strong CI/testing-cloud posture through existing Sauce
  test workflows.
- **Evidence artifacts and replay:** Sauce session outputs plus axe results are
  platform artifacts; Allie-style packet/release projection is not the public
  contract.
- **Manual review workflow:** Primarily automation/cloud workflow; manual review
  is not the core cited differentiator.
- **AI/agentic claims:** Not the primary cited accessibility claim.
- **Privacy/governance posture:** Secure/scalable cloud positioning.
- **Allie differentiation:** Keep Sauce as a runner option, not the source of
  accessibility policy or release semantics.
- **Sources:** [Sauce accessibility testing](https://saucelabs.com/products/accessibility-testing),
  [Sauce Deque integration docs](https://docs.saucelabs.com/basics/integrations/deque/),
  [Deque and Sauce guide](https://www.deque.com/saucelabs/get-started/)

### Stark

- **Category:** Design-to-code accessibility platform.
- **Automation depth:** Integrated tools across design, code, live product, WCAG
  audits, source-code scanning, and compliance workflows.
- **Standards claims:** WCAG audit and compliance positioning across product and
  design workflows.
- **CI/PR integration:** Source-code scanning and developer tooling, including
  CLI/ESLint positioning in public materials.
- **Evidence artifacts and replay:** Dashboard and report workflows; runtime
  browser replay is not the core contract.
- **Manual review workflow:** Product and compliance center workflows help teams
  manage accessibility posture.
- **AI/agentic claims:** Public extension/store language includes smart
  suggestions; core differentiation is broader end-to-end workflow.
- **Privacy/governance posture:** Platform compliance-center posture.
- **Allie differentiation:** Focus on application-runtime evidence, replayable
  browser artifacts, and release-gate semantics after code is running.
- **Sources:** [Stark](https://www.getstark.co/),
  [Source code scanning](https://www.getstark.co/blog/source-code-scanning/),
  [Stark blog](https://www.getstark.co/blog/)

### WAVE and WebAIM

- **Category:** Browser extension, online scanner, API, and evaluation aid.
- **Automation depth:** Page analysis through web UI, extensions, API, and
  stand-alone testing engine.
- **Standards claims:** Accessibility evaluation support; WAVE explicitly warns
  that no automated tool can tell whether a page is accessible.
- **CI/PR integration:** API and stand-alone engine can be integrated into
  infrastructure, including CI.
- **Evidence artifacts and replay:** Visual page feedback, API data, and testing
  engine outputs; replay depends on the calling system.
- **Manual review workflow:** Strong human-evaluation aid with visual indicators
  and education-oriented feedback.
- **AI/agentic claims:** None in the cited official pages.
- **Privacy/governance posture:** Browser extension runs entirely in the browser
  and is described as suitable for intranet, password-protected, and sensitive
  pages.
- **Allie differentiation:** Import scanner-style evidence while preserving
  product-flow replay, artifact graphs, and release policy context.
- **Sources:** [WAVE](https://wave.webaim.org/),
  [WAVE API](https://wave.webaim.org/api/),
  [WAVE extension](https://wave.webaim.org/extension/)

### Siteimprove

- **Category:** Enterprise governance, monitoring, content intelligence, and
  accessibility platform.
- **Automation depth:** Platform audits and monitors accessibility across sites;
  Siteimprove has added AI-supported accessibility rules.
- **Standards claims:** Public accessibility software page names WCAG 2.1 and
  2.2 at A, AA, and AAA, plus ADA, Section 508, and EAA context.
- **CI/PR integration:** Platform and dashboard posture rather than a portable
  local CLI contract.
- **Evidence artifacts and replay:** Dashboards, reports, scores, and tracked
  issues; replayable local packets are not the primary artifact.
- **Manual review workflow:** Enterprise accessibility workflow and issue
  guidance, not a code-repo-first audit packet.
- **AI/agentic claims:** AI-supported rules to expand WCAG coverage and improve
  detection accuracy are public in help content.
- **Privacy/governance posture:** Enterprise platform governance and ACR-facing
  posture.
- **Allie differentiation:** Stay narrower and deeper for engineering release
  evidence instead of broad content-program management.
- **Sources:** [Siteimprove accessibility software](https://www.siteimprove.com/platform/accessibility/web-accessibility-software/),
  [Siteimprove compliance](https://www.siteimprove.com/web-accessibility-compliance/),
  [AI-supported rules](https://help.siteimprove.com/support/solutions/articles/80001183313-expanding-wcag-coverage-with-new-ai-supported-rules)

### IBM Equal Access Toolkit

- **Category:** Open accessibility lifecycle guidance and checker tooling.
- **Automation depth:** Toolkit spans plan/design/develop/verify/launch; verify
  includes automated, manual, and screen-reader testing guidance.
- **Standards claims:** Equal Access tooling topics include WCAG 2.0, 2.1, 2.2,
  and Section 508.
- **CI/PR integration:** Equal Access Accessibility Checker includes browser and
  continuous development/build environment tooling.
- **Evidence artifacts and replay:** Checker outputs and lifecycle guidance;
  no Allie-style evidence packet/release projection contract.
- **Manual review workflow:** Manual and screen-reader verification are explicit
  parts of the toolkit.
- **AI/agentic claims:** None in the cited official sources.
- **Privacy/governance posture:** Open toolkit and local tooling posture.
- **Allie differentiation:** Learn from the lifecycle framing while making each
  verification artifact executable, replayable, and release-aware.
- **Sources:** [IBM Equal Access Toolkit](https://www.ibm.com/able/toolkit/),
  [Verify automated](https://www.ibm.com/able/toolkit/verify/automated/),
  [Equal Access GitHub](https://github.com/ibma/equal-access)

### W3C WAI tools list and ACT Rules

- **Category:** Neutral ecosystem directory and testing-rule standardization.
- **Automation depth:** W3C lists many evaluation tools; ACT Rules cover
  automated, semi-automated, and manual testing methods.
- **Standards claims:** ACT Rules are designed for conformance testing against
  WCAG, ARIA, and other accessibility practices.
- **CI/PR integration:** W3C is not a runner; tools and implementers use the
  rules and directory.
- **Evidence artifacts and replay:** Rules include test cases and examples, but
  do not prescribe an Allie packet shape.
- **Manual review workflow:** ACT explicitly includes manual and
  semi-automated testing.
- **AI/agentic claims:** None; this is standards and ecosystem guidance, not an
  AI product claim.
- **Privacy/governance posture:** Standards and directory posture.
- **Allie differentiation:** Use W3C materials to make coverage gaps explicit
  instead of implying automation can prove every success criterion.
- **Sources:** [W3C evaluation tools list](https://www.w3.org/WAI/test-evaluate/tools/list/),
  [ACT overview](https://www.w3.org/WAI/standards-guidelines/act/),
  [ACT Rules](https://www.w3.org/WAI/standards-guidelines/act/rules/)

### TPGi ARC Platform and ARC Toolkit

- **Category:** Enterprise accessibility platform plus browser-testing toolkit.
- **Automation depth:** ARC Platform includes automated monitoring, user-flow
  tests, program management, integrations, and knowledge base; ARC Toolkit is a
  page-level extension.
- **Standards claims:** ARC Toolkit targets WCAG 2.1 Level A and AA; monitoring
  pages discuss machine-detectable WCAG 2.0 and 2.1 defects.
- **CI/PR integration:** ARC Platform describes integrations and continuous
  accessibility data flows; API material references CI/CD use.
- **Evidence artifacts and replay:** Dashboards, monitoring data, findings, and
  knowledge-base guidance; replay semantics belong to ARC workflows.
- **Manual review workflow:** ARC consolidates automated and manual audit data in
  one central location.
- **AI/agentic claims:** Not the cited public differentiator.
- **Privacy/governance posture:** Enterprise program-management posture.
- **Allie differentiation:** Compete by keeping the developer-facing evidence
  packet and release decision portable before enterprise dashboard adoption.
- **Sources:** [ARC Platform](https://www.tpgi.com/arc-platform/),
  [ARC Toolkit](https://www.tpgi.com/arc-platform/arc-toolkit/),
  [ARC Monitoring](https://www.tpgi.com/arc-platform/monitoring/)

### Applitools Contrast Advisor

- **Category:** Visual AI testing with accessibility contrast analysis.
- **Automation depth:** Visual AI examines web, mobile web, native mobile,
  desktop, PDFs, graphics, icons, UI components, and text for contrast guidance.
- **Standards claims:** Documentation says Contrast Advisor helps comply with
  WCAG visual-impairment requirements and focuses on contrast ratios.
- **CI/PR integration:** Applitools fits existing visual test automation
  workflows.
- **Evidence artifacts and replay:** Visual test baselines and contrast findings
  are Applitools artifacts; Allie-style standards matrix and release projection
  remain external.
- **Manual review workflow:** Visual differences and contrast findings still
  require review in test workflows.
- **AI/agentic claims:** Explicit Visual AI positioning.
- **Privacy/governance posture:** Hosted visual testing platform posture.
- **Allie differentiation:** Use visual AI as one evidence class, then connect
  findings to WCAG criteria, product surfaces, redaction, and release policy.
- **Sources:** [Contrast Advisor](https://applitools.com/platform/validate/accessibility/),
  [Contrast Advisor docs](https://applitools.com/docs/eyes/concepts/test-execution/accessibility-testing),
  [Visual AI accessibility](https://applitools.com/lp/accessibility-testing/)

## Differentiation Bets

1. **Evidence packet as the product interface.** Every scanner, agent, browser
   run, manual review, waiver, and release projection should project into the
   same versioned packet graph.
2. **Replay before enforcement.** Generated or agentic findings remain advisory
   until replayed through deterministic/scripted evidence or promoted by human
   attestation.
3. **Criterion-by-surface visibility.** The report starts from WCAG criteria and
   product surfaces, not from whatever a scanner happened to find.
4. **Local-first and host-agnostic.** GitHub, Azure, browser clouds, and hosted
   dashboards are adapters over local artifacts, not the only way to use Allie.
5. **Governed AI.** Vision and LLM output should have model metadata, prompt and
   response artifacts, redaction receipts, confidence, and promotion state.
6. **No false legal guarantee.** Allie reports evidence, gaps, status,
   confidence, and residual human review needs.

## Landscape Review Checklist

Run this checklist before V1 positioning changes, hosted/dashboard work, pricing
or packaging decisions, and after any real-app dogfood run:

- Refresh `Last reviewed` with the review date.
- Add or remove vendors only with a source URL and an Allie differentiation
  note.
- For every entry, re-check automation depth, standards claims, CI/PR
  integration, artifacts/replay, manual workflow, AI claims, and governance
  posture.
- Mark claims as public-source observations, not legal or procurement advice.
- Run `npm run landscape:smoke` and keep the output with the PR evidence.

## Roadmap Prioritization Rules

- Prefer work that strengthens Allie's packet/replay/release-governance position
  over scanner-rule parity.
- Treat browser clouds, visual AI, and enterprise dashboards as adapters or
  projections until the local evidence contract is strong.
- Add AI/agentic capability only when it carries redaction, model metadata,
  confidence, promotion state, and human-review semantics.
- Do not build hosted dashboards before the host-agnostic CLI and artifact
  contract are easy to dogfood locally.
- If a competitor already solves a proposed workflow well, backlog the Allie
  version only when it materially improves provenance, replay, privacy, or
  standards drilldown.

## Watchlist

- Deque AI-assisted guided/manual testing depth and any public packet/export
  model changes.
- Evinced AI detection and clustering claims, especially whether findings become
  CI blockers and how evidence is retained.
- Level Access, TPGi ARC, Siteimprove, and Stark compliance-center evidence
  models.
- BrowserStack, Sauce Labs, and Applitools accessibility artifacts, screen-reader
  workflows, cloud export formats, and AI positioning.
- WAVE, IBM Equal Access, Accessibility Insights, Pa11y, and W3C ACT rules as
  open or neutral sources for evidence and coverage discipline.

## Review Cadence

Refresh this file before major V1 positioning changes, before hosted/dashboard
work, after dogfood in a real app, and at least monthly while Allie is actively
being shaped.
