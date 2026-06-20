#!/bin/sh
set -eu

DOC=${DOC:-docs/competitive-landscape.md}
ROADMAP=${ROADMAP:-docs/roadmap.md}

test -f "$DOC"
test -f "$ROADMAP"

node - "$DOC" "$ROADMAP" <<'NODE'
const fs = require('fs');
const path = process.argv[2];
const roadmapPath = process.argv[3];
const doc = fs.readFileSync(path, 'utf8');
const roadmap = fs.readFileSync(roadmapPath, 'utf8');
const requiredFields = [
  'Category',
  'Automation depth',
  'Standards claims',
  'CI/PR integration',
  'Evidence artifacts and replay',
  'Manual review workflow',
  'AI/agentic claims',
  'Privacy/governance posture',
  'Allie differentiation',
  'Sources'
];
const linkPattern = /\[[^\]]+\]\(https?:\/\/[^)]+\)/g;
const allowedSourceUrls = new Set([
  'https://github.com/dequelabs/axe-core',
  'https://www.deque.com/axe/axe-core/',
  'https://www.deque.com/axe/',
  'https://www.deque.com/axe/devtools/extension/',
  'https://docs.deque.com/devtools-for-web/4/en/devtools-igt/',
  'https://www.evinced.com/',
  'https://www.evinced.com/technology',
  'https://www.evinced.com/blog/introduction-to-test-automation-for-accessibility-managers',
  'https://www.levelaccess.com/',
  'https://www.levelaccess.com/accessibility-statement/',
  'https://www.levelaccess.com/blog/automated-accessibility-testing-a-practical-guide-to-wcag-coverage/',
  'https://accessibilityinsights.io/docs/web/overview/',
  'https://accessibilityinsights.io/docs/web/getstarted/assessment/',
  'https://accessibilityinsights.io/docs/web/reference/faq/',
  'https://pa11y.org/',
  'https://github.com/pa11y/pa11y',
  'https://github.com/pa11y/pa11y-ci',
  'https://developer.chrome.com/docs/lighthouse/overview',
  'https://developer.chrome.com/docs/lighthouse/accessibility/scoring',
  'https://github.com/GoogleChrome/lighthouse',
  'https://www.browserstack.com/accessibility-testing',
  'https://www.browserstack.com/docs/automate/selenium/accessibility-testing',
  'https://www.browserstack.com/accessibility-testing/ai-agents',
  'https://saucelabs.com/products/accessibility-testing',
  'https://docs.saucelabs.com/basics/integrations/deque/',
  'https://www.deque.com/saucelabs/get-started/',
  'https://www.getstark.co/',
  'https://www.getstark.co/blog/source-code-scanning/',
  'https://www.getstark.co/blog/',
  'https://wave.webaim.org/',
  'https://wave.webaim.org/api/',
  'https://wave.webaim.org/extension/',
  'https://www.siteimprove.com/platform/accessibility/web-accessibility-software/',
  'https://www.siteimprove.com/web-accessibility-compliance/',
  'https://help.siteimprove.com/support/solutions/articles/80001183313-expanding-wcag-coverage-with-new-ai-supported-rules',
  'https://www.ibm.com/able/toolkit/',
  'https://www.ibm.com/able/toolkit/verify/automated/',
  'https://github.com/ibma/equal-access',
  'https://www.w3.org/WAI/test-evaluate/tools/list/',
  'https://www.w3.org/WAI/standards-guidelines/act/',
  'https://www.w3.org/WAI/standards-guidelines/act/rules/',
  'https://www.tpgi.com/arc-platform/',
  'https://www.tpgi.com/arc-platform/arc-toolkit/',
  'https://www.tpgi.com/arc-platform/monitoring/',
  'https://applitools.com/platform/validate/accessibility/',
  'https://applitools.com/docs/eyes/concepts/test-execution/accessibility-testing',
  'https://applitools.com/lp/accessibility-testing/',
]);

const reviewed = doc.match(/^Last reviewed: (\d{4}-\d{2}-\d{2})$/m);
if (!reviewed) {
  throw new Error('competitive landscape missing Last reviewed date');
}
const reviewedAt = new Date(`${reviewed[1]}T00:00:00Z`);
const maxAgeDays = 45;
const now = process.env.ALLIE_LANDSCAPE_NOW
  ? new Date(process.env.ALLIE_LANDSCAPE_NOW)
  : new Date();
const ageDays = Math.floor((now - reviewedAt) / (24 * 60 * 60 * 1000));
if (ageDays < 0 || ageDays > maxAgeDays) {
  throw new Error(`competitive landscape review date is stale: ${reviewed[1]}`);
}

const sections = [...doc.matchAll(/^### (.+)$/gm)];
if (sections.length < 12) {
  throw new Error(`expected at least 12 landscape entries, found ${sections.length}`);
}

for (let index = 0; index < sections.length; index += 1) {
  const title = sections[index][1].trim();
  const start = sections[index].index;
  const end = index + 1 < sections.length ? sections[index + 1].index : doc.length;
  const section = doc.slice(start, end);
  for (const field of requiredFields) {
    const fieldBlock = section.match(
      new RegExp(`- \\*\\*${escapeRegex(field)}:\\*\\* ([\\s\\S]*?)(?=\\n- \\*\\*|\\n### |\\n## |$)`)
    );
    if (!fieldBlock) {
      throw new Error(`${title} missing field: ${field}`);
    }
    const value = fieldBlock[1].replace(linkPattern, '').trim();
    if (field !== 'Sources' && value.length < 12) {
      throw new Error(`${title} field has no substantive value: ${field}`);
    }
  }
  const sourcesBlock = section.match(
    /- \*\*Sources:\*\* ([\s\S]*?)(?=\n- \*\*|\n### |\n## |$)/
  );
  const sourceLinks = sourcesBlock ? sourcesBlock[1].match(linkPattern) || [] : [];
  if (sourceLinks.length < 2) {
    throw new Error(`${title} must cite at least two source URLs`);
  }
  for (const link of sourceLinks) {
    const url = link.match(/\((https?:\/\/[^)]+)\)/)[1];
    if (!allowedSourceUrls.has(url)) {
      throw new Error(`${title} cites unapproved source URL: ${url}`);
    }
  }
  const differentiation = section.match(/- \*\*Allie differentiation:\*\* (.+)/);
  if (!differentiation || differentiation[1].trim().length < 24) {
    throw new Error(`${title} needs a substantive Allie differentiation note`);
  }
}

if (!doc.includes('## Landscape Review Checklist')) {
  throw new Error('missing recurring landscape review checklist');
}
if (!doc.includes('## Roadmap Prioritization Rules')) {
  throw new Error('missing roadmap prioritization rules');
}
if (!roadmap.includes('competitive-landscape.md')) {
  throw new Error('roadmap missing competitive landscape cross-reference');
}
for (const phrase of ['packet provenance', 'replayability', 'privacy governance']) {
  if (!roadmap.includes(phrase)) {
    throw new Error(`roadmap missing landscape prioritization phrase: ${phrase}`);
  }
}

console.log(`landscape smoke passed: ${sections.length} entries, reviewed ${reviewed[1]}`);

function escapeRegex(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
NODE
