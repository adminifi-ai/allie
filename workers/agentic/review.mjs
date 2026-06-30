#!/usr/bin/env node
// Allie agentic review gateway.
//
// Given the criteria that need visual/contextual judgment, this captures the
// evidence a human reviewer needs — a fresh screenshot, a focus-state montage,
// and short focus/motion clips — and asks a vision model (via OpenRouter) for a
// structured per-criterion assessment plus reviewer guidance. Provider details
// stay isolated here; Rust owns which criteria, the budget, and the policy.
//
// It never fabricates a verdict: if the model is unavailable or a call fails,
// the affected criteria come back as "unavailable" with the captured media
// still attached, and the response status is "degraded".
import fs from 'node:fs/promises';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';

const REQUEST_SCHEMA = 'allie.agentic.request.v0';
const RESPONSE_SCHEMA = 'allie.agentic.response.v0';
const MAX_REVIEW_ACTIONS = 3;
const ALLOWED_REVIEW_KEYS = new Set(['Tab', 'Escape', 'Enter', 'Space', 'ArrowDown', 'ArrowUp']);
const moduleDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(moduleDir, '../..');

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const request = JSON.parse(await fs.readFile(path.resolve(repoRoot, args.request), 'utf8'));
  const response = await run(request);
  const out = path.resolve(repoRoot, args.response);
  await fs.mkdir(path.dirname(out), { recursive: true });
  await fs.writeFile(out, `${JSON.stringify(response, null, 2)}\n`);
  if (response.status === 'error') process.exitCode = 1;
}

async function run(request) {
  if (request.schema !== REQUEST_SCHEMA) {
    return errorResponse(`unexpected request schema ${request.schema}`);
  }
  const apiKey = process.env[request.model.api_key_env || 'OPENROUTER_API_KEY'];
  const artifactsDir = path.resolve(repoRoot, request.artifacts_dir);
  await fs.mkdir(artifactsDir, { recursive: true });

  let browser = null;
  let fixtureServer = null;
  const errors = [];
  try {
    const target = await resolveTarget(request.target);
    fixtureServer = target.server;
    browser = await chromium.launch({ headless: true });

    // Capture the visual evidence the reviewer (and the model) will use.
    const contextOptions = browserContextOptions(request.browser);
    const media = await captureEvidence(browser, target.baseUrl, contextOptions, artifactsDir, errors);

    const groups = groupCriteria(request.criteria || []);
    const maxCalls = request.model.max_calls ?? 4;
    let calls = 0;
    const usage = { prompt_tokens: 0, completion_tokens: 0 };
    const assessments = [];

    for (const group of groups) {
      if (group.items.length === 0) continue;
      let groupMedia = mediaForGroup(group.kind, media);
      // The model sees the full page plus criterion-specific screenshots/clips;
      // the report attaches only criterion-specific media since the full page
      // already shows once in the report's "what Allie inspected" gallery and
      // motion frames are model-only (the clip is friendlier for a human) —
      // avoids inlining the same or near-identical screenshots dozens of times.
      const verdicts = {};
      for (const batch of chunk(group.items, 8)) {
        if (apiKey && calls < maxCalls) {
          try {
            const result = await assessGroupWithReviewLoop(request.model, apiKey, { ...group, items: batch }, groupMedia, {
              browser,
              baseUrl: target.baseUrl,
              contextOptions,
              artifactsDir,
              errors,
              budget: maxCalls - calls,
            });
            Object.assign(verdicts, result.verdicts);
            groupMedia = result.media;
            calls += result.calls;
            usage.prompt_tokens += result.usage?.prompt_tokens || 0;
            usage.completion_tokens += result.usage?.completion_tokens || 0;
          } catch (error) {
            errors.push(`model call for ${group.kind} failed: ${error.message}`);
          }
        } else if (!apiKey) {
          errors.push('no model API key configured; criteria captured but not AI-assessed');
        } else {
          errors.push(`model-call budget (${maxCalls}) exhausted before finishing ${group.kind} group`);
        }
      }
      const reportMedia = groupMedia.filter((entry) => entry !== media.fullpage && !entry.modelOnly);
      for (const item of group.items) {
        const verdict = verdicts[item.obligation];
        // Never fabricate a pass/fail: a missing/unparseable verdict stays
        // "inconclusive" so Rust keeps the criterion at needs_review.
        const rawVerdict = (verdict?.verdict || '').toLowerCase();
        const settled = rawVerdict === 'pass' || rawVerdict === 'fail';
        assessments.push({
          obligation: item.obligation,
          verdict: settled ? rawVerdict : 'inconclusive',
          confidence: settled ? (verdict?.confidence || 'low') : 'not_observed',
          rationale: verdict?.rationale || 'Agentic review did not return a verdict for this criterion; the captured evidence is attached for human review.',
          reviewer_guidance: verdict?.reviewer_guidance || 'Review the attached evidence manually against this criterion.',
          media: reportMedia.map((entry) => ({
            kind: entry.kind,
            caption: entry.caption,
            path: path.relative(artifactsDir, entry.absPath).split(path.sep).join('/'),
          })),
        });
      }
    }

    await browser.close();
    browser = null;

    const status = errors.length === 0 ? 'ok' : 'degraded';
    return {
      schema: RESPONSE_SCHEMA,
      status,
      provider: request.model.provider || 'openrouter',
      model: request.model.model,
      calls,
      usage,
      assessments,
      errors,
    };
  } catch (error) {
    return errorResponse(error instanceof Error ? error.message : String(error));
  } finally {
    if (browser) await browser.close().catch(() => {});
    if (fixtureServer) await new Promise((resolve) => fixtureServer.close(resolve)).catch(() => {});
  }
}

// --- evidence capture -------------------------------------------------------

function browserContextOptions(browserSettings) {
  return {
    viewport: browserSettings.viewport,
    colorScheme: browserSettings.color_scheme,
    reducedMotion: browserSettings.reduced_motion,
    locale: browserSettings.locale,
  };
}

async function captureEvidence(browser, baseUrl, contextOptions, artifactsDir, errors) {
  const media = { fullpage: null, focus: [], focusClip: null, motionClip: null, motionFrames: [] };

  // Full-page screenshot.
  const context = await browser.newContext(contextOptions);
  const page = await context.newPage();
  await page.goto(baseUrl, { waitUntil: 'networkidle' });
  const fullpagePath = path.join(artifactsDir, 'review-fullpage.png');
  await page.screenshot({ path: fullpagePath, fullPage: true });
  media.fullpage = { kind: 'screenshot', caption: 'Full page as the AI reviewer saw it', absPath: fullpagePath };

  // Focus-state montage: tab through and screenshot the focused viewport.
  for (let i = 0; i < 6; i += 1) {
    await page.keyboard.press('Tab');
    const label = await page.evaluate(() => {
      const el = document.activeElement;
      if (!el || el === document.body) return 'body';
      return (el.getAttribute('aria-label') || el.textContent || el.tagName || '').trim().replace(/\s+/g, ' ').slice(0, 40);
    });
    if (label === 'body') break;
    const focusPath = path.join(artifactsDir, `review-focus-${i + 1}.png`);
    await page.screenshot({ path: focusPath });
    media.focus.push({ kind: 'screenshot', caption: `Keyboard focus on: ${label}`, absPath: focusPath });
  }
  await context.close();

  // Focus clip: record tabbing through the page.
  media.focusClip = await recordClip(browser, baseUrl, contextOptions, artifactsDir, 'review-focus-clip', async (clipPage) => {
    for (let i = 0; i < 8; i += 1) {
      await clipPage.keyboard.press('Tab');
      await clipPage.waitForTimeout(220);
    }
  }, 'Keyboard focus moving through the page', errors);

  // Motion clip: let the page sit so any animation/auto-updating content plays.
  media.motionClip = await recordClip(browser, baseUrl, contextOptions, artifactsDir, 'review-motion-clip', async (clipPage) => {
    await clipPage.waitForTimeout(2600);
  }, 'The page over ~2.5s (motion / auto-updating content)', errors);

  // Motion montage: sample still frames over ~2.4s with motion ENABLED (as a
  // user with no reduced-motion preference would see it), so the vision model
  // can compare frames and actually judge motion / animation / auto-updating
  // content — a single static screenshot cannot show movement. These frames are
  // model-only; the report keeps the clip (better for a human than four stills).
  // Note: this DELIBERATELY overrides the manifest's reduced_motion (which the
  // motion clip above honors) — judging 2.2.x needs motion present to observe.
  try {
    const motionContext = await browser.newContext({ ...contextOptions, reducedMotion: 'no-preference' });
    const motionPage = await motionContext.newPage();
    await motionPage.goto(baseUrl, { waitUntil: 'networkidle' });
    for (let i = 0; i < 4; i += 1) {
      const framePath = path.join(artifactsDir, `review-motion-frame-${i + 1}.png`);
      await motionPage.screenshot({ path: framePath });
      media.motionFrames.push({
        kind: 'screenshot',
        caption: `Motion frame ${i + 1} (t≈${(i * 0.8).toFixed(1)}s)`,
        absPath: framePath,
        modelOnly: true,
      });
      if (i < 3) await motionPage.waitForTimeout(800);
    }
    await motionContext.close();
  } catch (error) {
    errors.push(`motion montage failed: ${error.message}`);
  }

  return media;
}

async function recordClip(browser, baseUrl, contextOptions, artifactsDir, name, actions, caption, errors) {
  try {
    const context = await browser.newContext({
      ...contextOptions,
      recordVideo: { dir: artifactsDir, size: contextOptions.viewport },
    });
    const page = await context.newPage();
    await page.goto(baseUrl, { waitUntil: 'networkidle' });
    await actions(page);
    const video = page.video();
    await context.close();
    if (!video) return null;
    const src = await video.path();
    const dest = path.join(artifactsDir, `${name}.webm`);
    await fs.copyFile(src, dest);
    await fs.rm(src, { force: true }).catch(() => {});
    return { kind: 'clip', caption, absPath: dest };
  } catch (error) {
    errors.push(`clip ${name} failed: ${error.message}`);
    return null;
  }
}

function mediaForGroup(kind, media) {
  if (kind === 'focus') {
    return [media.fullpage, ...media.focus.slice(0, 3), media.focusClip].filter(Boolean);
  }
  if (kind === 'motion') {
    // Frames first for broad compatibility, with the clip included for
    // video-capable models and for the human report.
    return [...media.motionFrames, media.motionClip].filter(Boolean);
  }
  return [media.fullpage].filter(Boolean);
}

// --- model boundary ---------------------------------------------------------

async function assessGroupWithReviewLoop(model, apiKey, group, groupMedia, context) {
  let media = groupMedia;
  let finalResult = { verdicts: {}, usage: {}, actions: [] };
  const usage = { prompt_tokens: 0, completion_tokens: 0 };
  let calls = 0;

  for (let attempt = 0; attempt < 2 && calls < context.budget; attempt += 1) {
    const result = await assessGroup(model, apiKey, { ...group, rejudge_attempt: attempt }, media, context.errors);
    calls += 1;
    finalResult = result;
    usage.prompt_tokens += result.usage?.prompt_tokens || 0;
    usage.completion_tokens += result.usage?.completion_tokens || 0;

    const requestedActionCount = Array.isArray(result.actions) ? result.actions.length : 0;
    const actions = safeReviewActions(result.actions);
    if (requestedActionCount > 0 && actions.length === 0) {
      context.errors.push(`model requested unsupported review actions for ${group.kind}; ignored`);
    }
    if (attempt === 0 && actions.length > 0 && calls >= context.budget) {
      context.errors.push(`model requested review actions for ${group.kind}, but model-call budget was exhausted`);
    }
    if (attempt === 0 && actions.length > 0 && calls < context.budget) {
      const actionMedia = await captureReviewActionMedia({
        ...context,
        groupKind: group.kind,
        attempt: attempt + 1,
        actions,
      });
      if (actionMedia.length > 0) {
        media = [...media, ...actionMedia];
        continue;
      }
    }
    break;
  }

  return { verdicts: finalResult.verdicts, usage, calls, media };
}

async function captureReviewActionMedia({ browser, baseUrl, contextOptions, artifactsDir, errors, groupKind, attempt, actions }) {
  let context = null;
  try {
    context = await browser.newContext(contextOptions);
    const page = await context.newPage();
    await page.goto(baseUrl, { waitUntil: 'networkidle' });
    for (const action of actions) {
      if (action.type === 'press_key') {
        await page.keyboard.press(action.key);
        await page.waitForTimeout(180);
      } else if (action.type === 'wait_ms') {
        await page.waitForTimeout(action.ms);
      }
    }
    const actionPath = path.join(artifactsDir, `review-action-${groupKind}-${attempt}.png`);
    await page.screenshot({ path: actionPath });
    return [{
      kind: 'screenshot',
      caption: `Review action ${attempt}: ${actionSummary(actions)}`,
      absPath: actionPath,
    }];
  } catch (error) {
    errors.push(`review action for ${groupKind} failed: ${error.message}`);
    return [];
  } finally {
    if (context) await context.close().catch(() => {});
  }
}

function safeReviewActions(actions) {
  if (!Array.isArray(actions)) return [];
  const safe = [];
  for (const action of actions) {
    if (safe.length >= MAX_REVIEW_ACTIONS) break;
    if (action?.type === 'press_key' && ALLOWED_REVIEW_KEYS.has(action.key)) {
      safe.push({ type: 'press_key', key: action.key, reason: String(action.reason || '').slice(0, 120) });
    } else if (action?.type === 'wait_ms' && Number.isFinite(action.ms)) {
      safe.push({ type: 'wait_ms', ms: Math.max(100, Math.min(2000, Math.trunc(action.ms))) });
    }
  }
  return safe;
}

function actionSummary(actions) {
  return actions
    .map((action) => (action.type === 'press_key' ? `press ${action.key}` : `wait ${action.ms}ms`))
    .join(', ');
}

async function assessGroup(model, apiKey, group, groupMedia, errors) {
  const imageMedia = modelImageMedia(groupMedia);
  const videoMedia = groupMedia.filter(isVideoMedia).slice(0, 2);
  const content = [{ type: 'text', text: buildPrompt(group, [...imageMedia, ...videoMedia]) }];
  for (const entry of imageMedia) {
    content.push({
      type: 'image_url',
      image_url: { url: await mediaDataUrl(entry.absPath, 'image/png') },
    });
  }
  for (const entry of videoMedia) {
    content.push({
      type: 'video_url',
      video_url: { url: await mediaDataUrl(entry.absPath, videoMimeType(entry.absPath)) },
    });
  }
  const body = {
    model: model.model,
    max_tokens: 4000,
    temperature: 0.2,
    messages: [{ role: 'user', content }],
  };
  // Thinking-effort hint for models that support it (e.g. Gemini 3.x). Keeping
  // this explicit (low) avoids the pricier "medium" default while preserving
  // full coverage and decisive verdicts.
  if (model.reasoning_effort) body.reasoning = { effort: model.reasoning_effort };
  const base = model.base_url || 'https://openrouter.ai/api/v1';
  const res = await fetch(`${base}/chat/completions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${apiKey}`,
      'Content-Type': 'application/json',
      'HTTP-Referer': 'https://github.com/adminifi-ai/allie',
      'X-Title': 'Allie accessibility review',
    },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`HTTP ${res.status}: ${text.slice(0, 200)}`);
  }
  const json = await res.json();
  const text = json.choices?.[0]?.message?.content || '';
  const parsed = parseModelJson(text);
  const verdicts = {};
  for (const entry of parsed?.assessments || []) {
    if (entry.obligation) verdicts[entry.obligation] = entry;
  }
  if (Object.keys(verdicts).length === 0) {
    errors.push(`model returned no parseable assessments for ${group.kind}`);
  }
  return { verdicts, actions: parsed?.actions || [], usage: json.usage };
}

function modelImageMedia(groupMedia) {
  const screenshots = groupMedia.filter((entry) => entry.kind === 'screenshot');
  const actionScreenshots = screenshots.filter((entry) => entry.caption.startsWith('Review action'));
  const baselineScreenshots = screenshots.filter((entry) => !entry.caption.startsWith('Review action'));
  return [...actionScreenshots, ...baselineScreenshots].slice(0, 4);
}

async function mediaDataUrl(absPath, mimeType) {
  const bytes = await fs.readFile(absPath);
  return `data:${mimeType};base64,${bytes.toString('base64')}`;
}

function isVideoMedia(entry) {
  return ['clip', 'video', 'video_clip'].includes(entry.kind);
}

function videoMimeType(absPath) {
  const ext = path.extname(absPath).toLowerCase();
  if (ext === '.mp4') return 'video/mp4';
  if (ext === '.mov') return 'video/quicktime';
  return 'video/webm';
}

function buildPrompt(group, media = []) {
  const list = group.items
    .map((item) => `- ${item.obligation} | ${item.num} ${item.handle} (Level ${item.level}, ${item.principle})`)
    .join('\n');
  const mediaList = media
    .map((entry, index) => `- media ${index + 1}: ${entry.kind} | ${entry.caption}`)
    .join('\n');
  return [
    'You are an expert WCAG 2.2 AA accessibility auditor. You are doing the job a trained human reviewer does: looking at the captured visual evidence and rendering a DEFINITIVE judgment for each success criterion.',
    `Focus area for this batch: ${group.guidance}`,
    'For each criterion, make the same call a human expert would make from this evidence:',
    '- verdict: "pass" or "fail". You MUST commit to one. Use "inconclusive" ONLY when even an expert genuinely cannot judge from any amount of looking (e.g. a precise color-contrast ratio that requires a measurement tool). Do not hide behind "inconclusive" to avoid deciding.',
    '- confidence: "high" | "medium" | "low" — be honest. Low confidence is fine and is exactly how we mark a judgment call; it does NOT mean refuse to decide.',
    '- rationale: one to two sentences grounded in what you actually see.',
    '- reviewer_guidance: the exact thing a human should do to confirm or refute your verdict.',
    'If a small browser observation would let you make a better judgment, you may request at most three review actions before the final verdict. Allowed action shapes are {"type":"press_key","key":"Tab|Escape|Enter|Space|ArrowDown|ArrowUp","reason":"..."} and {"type":"wait_ms","ms":100-2000}. Do not request clicks, typing, form submission, navigation, or mutation.',
    'Do not claim legal compliance. Be specific and visual.',
    '',
    'Criteria:',
    list,
    '',
    'Media supplied:',
    mediaList || '- none',
    '',
    'Respond with ONLY a JSON object, no prose, of the form:',
    '{"actions":[{"type":"press_key","key":"Tab","reason":"optional bounded observation"}],"assessments":[{"obligation":"<id>","verdict":"pass|fail|inconclusive","confidence":"high|medium|low","rationale":"...","reviewer_guidance":"..."}]}',
  ].join('\n');
}

function parseModelJson(text) {
  const fenced = text.match(/```(?:json)?\s*([\s\S]*?)```/);
  const raw = fenced ? fenced[1] : text;
  const start = raw.indexOf('{');
  const end = raw.lastIndexOf('}');
  if (start === -1 || end === -1) return null;
  try {
    return JSON.parse(raw.slice(start, end + 1));
  } catch {
    return null;
  }
}

// --- criterion grouping -----------------------------------------------------

const FOCUS_OBLIGATIONS = new Set([
  'wcag22-aa:2.4.3-focus-order',
  'wcag22-aa:2.4.7-focus-visible',
  'wcag22-aa:2.4.11-focus-not-obscured-minimum',
  'wcag22-aa:2.1.2-no-keyboard-trap',
  'wcag22-aa:1.4.13-content-on-hover-or-focus',
]);
const MOTION_OBLIGATIONS = new Set([
  'wcag22-aa:2.2.1-timing-adjustable',
  'wcag22-aa:2.2.2-pause-stop-hide',
  'wcag22-aa:2.3.1-three-flashes-or-below-threshold',
  'wcag22-aa:2.5.4-motion-actuation',
]);

function groupCriteria(criteria) {
  const focus = [];
  const motion = [];
  const general = [];
  for (const item of criteria) {
    if (FOCUS_OBLIGATIONS.has(item.obligation)) focus.push(item);
    else if (MOTION_OBLIGATIONS.has(item.obligation)) motion.push(item);
    else general.push(item);
  }
  return [
    { kind: 'general', guidance: 'General perceivable/operable/understandable/robust review from the page screenshot.', items: general },
    { kind: 'focus', guidance: 'Keyboard focus visibility and order, using the focus montage and focus walkthrough clip.', items: focus },
    { kind: 'motion', guidance: 'Motion, animation, timing and auto-updating content. You are shown several still frames plus a short walkthrough video captured over ~2.5 seconds. Compare the frames and video: if they are static there is no moving/auto-updating/flashing content; if they differ, judge whether the motion can be paused, stopped, or hidden and whether it flashes more than three times per second.', items: motion },
  ];
}

// --- helpers ----------------------------------------------------------------

async function resolveTarget(target) {
  if (target.base_url) return { baseUrl: target.base_url, server: null };
  if (target.fixture_dir) {
    const dir = path.resolve(repoRoot, target.fixture_dir);
    const server = await startFixtureServer(dir);
    const { port } = server.address();
    return { baseUrl: `http://127.0.0.1:${port}/`, server };
  }
  throw new Error('agentic request target requires base_url or fixture_dir');
}

async function startFixtureServer(dir) {
  const root = await fs.realpath(dir);
  const server = http.createServer(async (req, res) => {
    try {
      const url = new URL(req.url ?? '/', 'http://127.0.0.1');
      const rel = url.pathname === '/' ? 'index.html' : decodeURIComponent(url.pathname).replace(/^\/+/, '');
      const file = path.resolve(root, rel);
      if (file !== root && !file.startsWith(`${root}${path.sep}`)) {
        res.writeHead(403);
        res.end('Forbidden');
        return;
      }
      res.writeHead(200);
      res.end(await fs.readFile(file));
    } catch {
      res.writeHead(404);
      res.end('Not found');
    }
  });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  return server;
}

function chunk(items, size) {
  const out = [];
  for (let i = 0; i < items.length; i += size) out.push(items.slice(i, i + size));
  return out;
}

function errorResponse(message) {
  return { schema: RESPONSE_SCHEMA, status: 'error', provider: 'openrouter', model: null, calls: 0, usage: {}, assessments: [], errors: [message] };
}

function parseArgs(args) {
  const parsed = {};
  for (let i = 0; i < args.length; i += 1) {
    if (args[i] === '--request') parsed.request = args[++i];
    else if (args[i] === '--response') parsed.response = args[++i];
    else throw new Error(`unexpected argument: ${args[i]}`);
  }
  if (!parsed.request || !parsed.response) throw new Error('Usage: review.mjs --request <req.json> --response <res.json>');
  return parsed;
}

main().catch((error) => {
  console.error(error instanceof Error ? error.stack : error);
  process.exit(2);
});
