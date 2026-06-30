#!/usr/bin/env node
import AxeBuilder from '@axe-core/playwright';
import fs from 'node:fs/promises';
import fsSync from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';

const WORKER_REQUEST_SCHEMA = 'allie.worker.request.v0';
const WORKER_RESPONSE_SCHEMA = 'allie.worker.response.v0';
const STATE_STEP_TIMEOUT_MS = 5000;
const moduleDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(moduleDir, '../..');

async function main() {
  const args = parseArgs(process.argv.slice(2));

  if (args.smoke) {
    await runSmoke(args.smoke);
    return;
  }

  if (!args.request || !args.response) {
    throw new Error('Usage: node workers/browser/run.mjs --request <request.json> --response <response.json>');
  }

  const request = await readJson(args.request);
  const response = await runWorker(request);
  await writeJson(args.response, response);
}

async function runSmoke(outDir) {
  const smokeOut = path.resolve(repoRoot, outDir);
  const smokeRequestPath = path.resolve(repoRoot, 'workers/browser/smoke-request.json');
  const request = await readJson(smokeRequestPath);
  request.artifacts_dir = path.join(smokeOut, 'artifacts');

  await fs.mkdir(smokeOut, { recursive: true });
  await writeJson(path.join(smokeOut, 'worker-request.json'), request);

  const response = await runWorker(request);
  await writeJson(path.join(smokeOut, 'worker-response.json'), response);

  if (response.status !== 'passed') {
    console.error(JSON.stringify(response, null, 2));
    process.exitCode = 1;
  } else {
    console.log(`worker smoke passed: ${path.join(smokeOut, 'worker-response.json')}`);
  }
}

async function runWorker(request) {
  if (request.schema !== WORKER_REQUEST_SCHEMA) {
    return errorResponse(`unexpected request schema ${request.schema}`);
  }

  let fixtureServer = null;
  let browser = null;

  try {
    const artifactsDir = path.resolve(repoRoot, request.artifacts_dir);
    await fs.mkdir(artifactsDir, { recursive: true });

    const target = await resolveTarget(request.target);
    fixtureServer = target.server;

    browser = await chromium.launch({ headless: true });
    const wantsVideo = request.states.some((state) => state.video);
    const contextOptions = {
      viewport: request.browser.viewport,
      colorScheme: request.browser.color_scheme,
      reducedMotion: request.browser.reduced_motion,
      locale: request.browser.locale,
    };
    if (wantsVideo) {
      contextOptions.recordVideo = {
        dir: path.join(artifactsDir, 'videos'),
        size: request.browser.viewport,
      };
    }

    // Authenticated audit: load a captured session (storageState hatch) or run
    // the form-login recipe against a throwaway page. Secret values are read from
    // this process's inherited env — they are never present in the request JSON.
    let usedStorageState = false;
    const auth = request.auth ?? null;
    const storageStatePath = resolveStorageState(auth);
    if (storageStatePath) {
      contextOptions.storageState = storageStatePath;
      usedStorageState = true;
    }

    const context = await browser.newContext(contextOptions);

    if (auth && !usedStorageState) {
      await performLogin(context, target.baseUrl, auth);
    }

    const states = [];
    for (const state of request.states) {
      states.push(await inspectState(context, target.baseUrl, state, artifactsDir, request.browser.zoom, auth?.authenticated_marker ?? null));
    }

    await context.close();
    await browser.close();
    browser = null;

    const hasViolations = states.some((state) => state.axe_violations.length > 0);
    const hasStateErrors = states.some((state) => state.state_errors.length > 0);
    return {
      schema: WORKER_RESPONSE_SCHEMA,
      status: hasViolations || hasStateErrors ? 'failed' : 'passed',
      actual_base_url: target.baseUrl,
      states,
      errors: [],
    };
  } catch (error) {
    return errorResponse(error instanceof Error ? error.message : String(error));
  } finally {
    if (browser) {
      await browser.close().catch(() => {});
    }
    if (fixtureServer) {
      await closeServer(fixtureServer).catch(() => {});
    }
  }
}

// Resolve the storageState hatch: an env var NAMES a path to a Playwright
// storageState file. Returns the path only when the env var is set and the file
// exists; otherwise null (the form-login recipe runs instead).
function resolveStorageState(auth) {
  const envName = auth?.storage_state_env;
  if (!envName) {
    return null;
  }
  const candidate = process.env[envName];
  if (!candidate) {
    return null;
  }
  if (!fsSync.existsSync(candidate) || !fsSync.statSync(candidate).isFile()) {
    return null;
  }
  return candidate;
}

// Run the form-login recipe once on a throwaway page. The JS-set session cookie
// persists in the shared context, so subsequent gated states are authenticated.
// Step values come from process.env[value_env]; no value is ever logged or
// thrown. On failure we throw `auth-failed at step N (<kind>)` with no secrets.
async function performLogin(context, baseUrl, auth) {
  const SHORT_TIMEOUT_MS = 10000;
  const page = await context.newPage();
  try {
    const startPath = auth.start_path ?? '/';
    await page.goto(new URL(startPath, baseUrl).toString(), { waitUntil: 'networkidle' });

    const steps = auth.steps ?? [];
    for (let index = 0; index < steps.length; index += 1) {
      const step = steps[index];
      try {
        if (step.fill) {
          const value = process.env[step.fill.value_env] ?? '';
          await page.fill(step.fill.selector, value);
        } else if (step.click) {
          await page.click(step.click.selector);
        } else if (step.wait_for) {
          await waitForCondition(page, step.wait_for, SHORT_TIMEOUT_MS);
        } else {
          throw new Error('unknown auth step');
        }
      } catch {
        // Never include the step value or env contents in the message.
        const kind = step.fill ? 'fill' : step.click ? 'click' : step.wait_for ? 'wait_for' : 'unknown';
        throw new Error(`auth-failed at step ${index} (${kind})`);
      }
    }
  } finally {
    await page.close();
  }
}

async function inspectState(context, baseUrl, state, artifactsDir, zoom, authMarker) {
  const page = await context.newPage();
  const pageVideo = page.video();
  const consoleErrors = [];
  const networkErrors = [];

  page.on('console', (message) => {
    if (message.type() === 'error') {
      consoleErrors.push(message.text());
    }
  });

  page.on('requestfailed', (request) => {
    networkErrors.push(`${request.method()} ${request.url()} ${request.failure()?.errorText ?? 'failed'}`);
  });

  const targetUrl = new URL(state.path, baseUrl).toString();
  const navigationResponse = await page.goto(targetUrl, { waitUntil: 'networkidle' });
  const navigationStatus = navigationResponse?.status() ?? null;
  const stateErrors = [];

  if (state.required && navigationStatus !== null && (navigationStatus < 200 || navigationStatus >= 400)) {
    stateErrors.push(`required route returned HTTP ${navigationStatus}`);
  }

  // No-silent-gaps: when an authenticated_marker is declared, a gated state must
  // show it (selector present and/or url_contains matches). An HTTP-200 SPA login
  // wall that bounced away from the gated route shows neither, so this records an
  // auth-lost state_error which flips the run to a blocking exit class
  // (lib.rs exit_class_for_response).
  if (authMarker) {
    const finalUrl = page.url();
    let markerPresent = true;
    if (authMarker.selector) {
      markerPresent = await page
        .waitForSelector(authMarker.selector, { timeout: 5000 })
        .then(() => true)
        .catch(() => false);
    }
    let urlOk = true;
    if (authMarker.url_contains) {
      urlOk = finalUrl.includes(authMarker.url_contains);
    }
    if (!markerPresent || !urlOk) {
      stateErrors.push(`auth-lost: authenticated marker not present (url ${finalUrl})`);
    }
  }

  await performStateSteps(page, state, stateErrors);

  if (zoom && zoom !== 1) {
    await page.evaluate((value) => {
      document.documentElement.style.zoom = String(value);
    }, zoom);
  }

  const finalUrl = page.url();
  const httpStatus = finalUrl === targetUrl ? navigationStatus : null;
  const title = await page.title();
  const keyboardFocusOrder = state.keyboard ? await captureKeyboardFocusOrder(page) : [];
  const screenshotPath = state.screenshot ? path.join(artifactsDir, `${state.id}.png`) : null;
  if (screenshotPath) {
    await page.screenshot({ path: screenshotPath, fullPage: true });
  }

  const domSnapshotPath = state.dom_snapshot ? path.join(artifactsDir, `dom-${state.id}.html`) : null;
  if (domSnapshotPath) {
    await fs.writeFile(domSnapshotPath, `${await page.content()}\n`);
  }

  let accessibilityTreePath = null;
  if (state.accessibility_tree) {
    accessibilityTreePath = path.join(artifactsDir, `accessibility-tree-${state.id}.json`);
    const tree = page.accessibility?.snapshot
      ? await page.accessibility.snapshot({ interestingOnly: false })
      : await page.evaluate(() => ({
        role: 'document',
        name: document.title,
        headings: [...document.querySelectorAll('h1,h2,h3,h4,h5,h6')].map((element) => ({
          level: Number(element.tagName.slice(1)),
          text: element.textContent?.trim() ?? '',
        })),
        controls: [...document.querySelectorAll('a,button,input,select,textarea,[role]')].map((element) => ({
          tag: element.tagName.toLowerCase(),
          role: element.getAttribute('role') || null,
          name: element.getAttribute('aria-label') || element.textContent?.trim() || element.getAttribute('name') || element.getAttribute('id') || '',
        })),
      }));
    await fs.writeFile(accessibilityTreePath, `${JSON.stringify(tree, null, 2)}\n`);
  }

  let axeJsonPath = null;
  let axeViolations = [];
  if (state.axe) {
    const axeResult = await new AxeBuilder({ page }).analyze();
    axeJsonPath = path.join(artifactsDir, `axe-${state.id}.json`);
    await fs.writeFile(axeJsonPath, `${JSON.stringify(axeResult, null, 2)}\n`);
    axeViolations = axeResult.violations.map((violation) => ({
      id: violation.id,
      impact: violation.impact ?? null,
      help: violation.help ?? null,
      description: violation.description ?? null,
      tags: violation.tags ?? [],
      nodes: violation.nodes?.length ?? 0,
    }));
  }

  const tracePath = state.trace ? path.join(artifactsDir, `trace-${state.id}.json`) : null;
  if (tracePath) {
    await fs.writeFile(tracePath, `${JSON.stringify({
      state: state.id,
      route: state.path,
      url: finalUrl,
      title,
      keyboard_focus_order: keyboardFocusOrder,
      console_errors: consoleErrors,
      network_errors: networkErrors,
    }, null, 2)}\n`);
  }

  const features = await captureFeatures(page);

  await page.close();
  let videoPath = null;
  if (state.video && pageVideo) {
    const candidateVideoPath = await pageVideo.path();
    const stableVideoPath = path.join(artifactsDir, `video-${state.id}.webm`);
    try {
      await fs.copyFile(candidateVideoPath, stableVideoPath);
      videoPath = stableVideoPath;
    } catch {
      videoPath = null;
    }
  }

  return {
    id: state.id,
    route: state.path,
    url: finalUrl,
    title,
    http_status: httpStatus,
    screenshot_path: screenshotPath ? path.relative(path.resolve(repoRoot, path.dirname(path.dirname(screenshotPath))), screenshotPath) : null,
    axe_json_path: axeJsonPath ? path.relative(path.resolve(repoRoot, path.dirname(path.dirname(axeJsonPath))), axeJsonPath) : null,
    dom_snapshot_path: domSnapshotPath ? path.relative(path.resolve(repoRoot, path.dirname(path.dirname(domSnapshotPath))), domSnapshotPath) : null,
    accessibility_tree_path: accessibilityTreePath ? path.relative(path.resolve(repoRoot, path.dirname(path.dirname(accessibilityTreePath))), accessibilityTreePath) : null,
    video_path: videoPath ? path.relative(path.resolve(repoRoot, path.dirname(path.dirname(videoPath))), videoPath) : null,
    trace_path: tracePath ? path.relative(path.resolve(repoRoot, path.dirname(path.dirname(tracePath))), tracePath) : null,
    keyboard_focus_order: keyboardFocusOrder,
    axe_violations: axeViolations,
    console_errors: consoleErrors,
    network_errors: networkErrors,
    state_errors: stateErrors,
    features,
  };
}

async function performStateSteps(page, state, stateErrors) {
  const steps = state.steps ?? [];
  for (let index = 0; index < steps.length; index += 1) {
    const step = steps[index];
    const kind = stateStepKind(step);
    try {
      if (step.fill) {
        await page.fill(step.fill.selector, step.fill.value ?? '', { timeout: STATE_STEP_TIMEOUT_MS });
      } else if (step.type) {
        await page.locator(step.type.selector).first().type(step.type.text ?? '', { timeout: STATE_STEP_TIMEOUT_MS });
      } else if (step.click) {
        await page.click(step.click.selector, { timeout: STATE_STEP_TIMEOUT_MS });
      } else if (step.wait_for || step.waitFor) {
        await waitForCondition(page, step.wait_for ?? step.waitFor, STATE_STEP_TIMEOUT_MS);
      } else {
        throw new Error('unknown state step');
      }
    } catch {
      stateErrors.push(`state-step-failed at step ${index} (${kind})`);
      return;
    }
  }
}

async function waitForCondition(page, waitFor, timeoutMs) {
  if (waitFor?.selector) {
    await page.waitForSelector(waitFor.selector, { timeout: timeoutMs });
  } else if (waitFor?.url_contains) {
    const fragment = waitFor.url_contains;
    await page.waitForURL((url) => url.toString().includes(fragment), { timeout: timeoutMs });
  } else {
    throw new Error('wait_for requires selector or url_contains');
  }
}

function stateStepKind(step) {
  if (step.fill) return 'fill';
  if (step.type) return 'type';
  if (step.click) return 'click';
  if (step.wait_for || step.waitFor) return 'wait_for';
  return 'unknown';
}

// Page feature inventory + lightweight scripted signals. Allie uses these to
// decide, automatically, which WCAG criteria are not applicable to the page
// (no audio/video, no forms, no draggable targets) and to run a couple of
// deterministic/scripted checks (page language, 320px reflow) so no criterion
// is left simply "not tested".
async function captureFeatures(page) {
  const counts = await page.evaluate(() => {
    const count = (selector) => document.querySelectorAll(selector).length;
    return {
      audio: count('audio'),
      video: count('video'),
      forms: count('form'),
      inputs: count('input:not([type=hidden]), select, textarea'),
      draggable: count('[draggable="true"]'),
      iframes: count('iframe'),
      images: count('img, svg[role="img"], [role="img"]'),
      links: count('a[href]'),
      headings: count('h1, h2, h3, h4, h5, h6'),
      lang: Boolean(document.documentElement.getAttribute('lang')),
      lang_value: document.documentElement.getAttribute('lang') || '',
    };
  });
  let reflowOverflow = false;
  let reflowChecked = false;
  try {
    const viewport = page.viewportSize();
    await page.setViewportSize({ width: 320, height: viewport?.height ?? 900 });
    reflowOverflow = await page.evaluate(
      () => document.documentElement.scrollWidth > document.documentElement.clientWidth + 2,
    );
    reflowChecked = true;
    if (viewport) {
      await page.setViewportSize(viewport);
    }
  } catch {
    reflowChecked = false;
  }
  return { ...counts, reflow_overflow: reflowOverflow, reflow_checked: reflowChecked };
}

async function captureKeyboardFocusOrder(page) {
  const seen = [];
  for (let index = 0; index < 12; index += 1) {
    await page.keyboard.press('Tab');
    const descriptor = await page.evaluate(() => {
      const element = document.activeElement;
      if (!element || element === document.body) return 'body';
      const tag = element.tagName.toLowerCase();
      const id = element.id ? `#${element.id}` : '';
      const label = element.getAttribute('aria-label') || element.textContent || element.getAttribute('name') || '';
      return `${tag}${id}:${label.trim().replace(/\s+/g, ' ').slice(0, 80)}`;
    });
    seen.push(descriptor);
  }
  return [...new Set(seen)];
}

async function resolveTarget(target) {
  if (target.kind === 'local_fixture') {
    if (!target.fixture_dir) {
      throw new Error('local_fixture target requires fixture_dir');
    }
    const fixtureDir = path.resolve(repoRoot, target.fixture_dir);
    const server = await startFixtureServer(fixtureDir);
    const { port } = server.address();
    return {
      baseUrl: `http://127.0.0.1:${port}/`,
      server,
    };
  }

  if (!target.base_url) {
    throw new Error('non-fixture target requires base_url');
  }

  return {
    baseUrl: target.base_url,
    server: null,
  };
}

async function startFixtureServer(fixtureDir) {
  const root = await fs.realpath(fixtureDir);
  const rootWithSeparator = root.endsWith(path.sep) ? root : `${root}${path.sep}`;

  const server = http.createServer(async (request, response) => {
    try {
      const requestUrl = new URL(request.url ?? '/', 'http://127.0.0.1');
      const relativePath = requestUrl.pathname === '/'
        ? 'index.html'
        : decodeURIComponent(requestUrl.pathname).replace(/^\/+/, '');
      const candidate = path.resolve(root, relativePath);

      if (candidate !== root && !candidate.startsWith(rootWithSeparator)) {
        response.writeHead(403);
        response.end('Forbidden');
        return;
      }

      const bytes = await fs.readFile(candidate);
      response.writeHead(200, { 'content-type': contentType(candidate) });
      response.end(bytes);
    } catch {
      response.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
      response.end('Not found');
    }
  });

  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });

  return server;
}

function contentType(filePath) {
  switch (path.extname(filePath)) {
    case '.html':
      return 'text/html; charset=utf-8';
    case '.css':
      return 'text/css; charset=utf-8';
    case '.js':
      return 'text/javascript; charset=utf-8';
    case '.json':
      return 'application/json; charset=utf-8';
    case '.png':
      return 'image/png';
    case '.svg':
      return 'image/svg+xml';
    default:
      return 'application/octet-stream';
  }
}

async function closeServer(server) {
  await new Promise((resolve, reject) => {
    server.close((error) => (error ? reject(error) : resolve()));
  });
}

function errorResponse(message) {
  return {
    schema: WORKER_RESPONSE_SCHEMA,
    status: 'error',
    actual_base_url: null,
    states: [],
    errors: [message],
    nondeterminism: [],
  };
}

async function readJson(filePath) {
  const text = await fs.readFile(path.resolve(repoRoot, filePath), 'utf8');
  return JSON.parse(text);
}

async function writeJson(filePath, value) {
  const resolved = path.resolve(repoRoot, filePath);
  await fs.mkdir(path.dirname(resolved), { recursive: true });
  await fs.writeFile(resolved, `${JSON.stringify(value, null, 2)}\n`);
}

function parseArgs(args) {
  const parsed = {};
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === '--request') {
      parsed.request = args[++index];
    } else if (arg === '--response') {
      parsed.response = args[++index];
    } else if (arg === '--smoke') {
      parsed.smoke = args[++index];
    } else {
      throw new Error(`unexpected argument: ${arg}`);
    }
  }
  return parsed;
}

main().catch((error) => {
  console.error(error instanceof Error ? error.stack : error);
  process.exit(2);
});
