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
const MAX_MODEL_RETRIES = 1;
const ALLOWED_REVIEW_KEYS = new Set(['Tab']);
// The macOS agent sandbox rejects Chromium's multiprocess Mach rendezvous.
// Agentic review opens isolated browsers for each capture segment because
// reusing one single-process browser across many contexts is unstable.
const CHROMIUM_LAUNCH_OPTIONS = { headless: true, args: ['--single-process'] };
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

  let fixtureServer = null;
  const errors = [];
  try {
    const target = await resolveTarget(request.target);
    fixtureServer = target.server;

    const contextOptions = browserContextOptions(request.browser);
    const surfaces = reviewSurfaces(request, target.baseUrl);
    const surfaceEvidence = [];
    for (const surface of surfaces) {
      surfaceEvidence.push(await captureEvidenceWithRetry(surface, contextOptions, artifactsDir, errors));
    }

    const groups = groupCriteria(request.criteria || []);
    const maxCalls = request.model.max_calls ?? 4;
    let calls = 0;
    const usage = { prompt_tokens: 0, completion_tokens: 0 };
    const assessmentResults = new Map();

    for (const group of groups) {
      if (group.items.length === 0) continue;
      for (const evidence of surfaceEvidence) {
        let groupMedia = mediaForGroup(group.kind, evidence);
        for (const batch of chunk(group.items, 8)) {
          const verdicts = {};
          if (apiKey && calls < maxCalls) {
            try {
              const result = await assessGroupWithReviewLoop(request.model, apiKey, { ...group, items: batch }, groupMedia, {
                surface: evidence.surface,
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
              calls += error.modelCalls || 0;
              errors.push(`model call for ${group.kind} on ${evidence.surface.id} failed: ${error.message}`);
            }
          } else if (!apiKey) {
            errors.push(`no model API key configured; ${evidence.surface.id} criteria captured but not AI-assessed`);
          } else {
            errors.push(`model-call budget (${maxCalls}) exhausted before finishing ${group.kind} on ${evidence.surface.id}`);
          }
          const reportMedia = groupMedia.filter((entry) => entry.role !== 'fullpage' && !entry.modelOnly);
          for (const item of batch) {
            recordSurfaceAssessment(assessmentResults, {
              item,
              surface: evidence.surface,
              verdict: verdicts[item.obligation],
              media: reportMedia,
            });
          }
        }
      }
    }
    const precisionGate = evaluatePrecisionGate(request.precision_gate, assessmentResults);
    if (precisionGate?.status === 'fail') {
      errors.push(`precision gate failed: ${precisionGate.fail_false_positives} false-positive FAIL(s), ${precisionGate.missing_labels} missing labeled result(s)`);
    }
    const assessments = finalizeAssessments(request.criteria || [], assessmentResults, artifactsDir);

    const status = errors.length === 0 ? 'ok' : 'degraded';
    return {
      schema: RESPONSE_SCHEMA,
      status,
      provider: request.model.provider || 'openrouter',
      model: request.model.model,
      calls,
      usage,
      assessments,
      ...(precisionGate ? { precision_gate: precisionGate } : {}),
      errors,
    };
  } catch (error) {
    return errorResponse(error instanceof Error ? error.message : String(error));
  } finally {
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

function reviewSurfaces(request, baseUrl) {
  const requested = Array.isArray(request.surfaces) ? request.surfaces : [];
  const surfaces = requested
    .map((surface, index) => normalizeSurface(surface, index, baseUrl))
    .filter(Boolean);
  if (surfaces.length > 0) return surfaces;
  return [normalizeSurface({ id: 'target', route: '/', title: 'Target page' }, 0, baseUrl)];
}

function normalizeSurface(surface, index, baseUrl) {
  const id = String(surface?.id || `surface-${index + 1}`).trim();
  const explicitRoute = String(surface?.route || surface?.path || '').trim();
  const rawUrl = String(surface?.url || explicitRoute || '/').trim();
  const navigationTarget = explicitRoute || rawUrl || '/';
  try {
    const url = new URL(navigationTarget, baseUrl);
    return {
      id,
      route: explicitRoute || url.pathname || '/',
      title: String(surface?.title || id).trim() || id,
      url: url.toString(),
      slug: slugify(id || explicitRoute || `surface-${index + 1}`),
    };
  } catch {
    return null;
  }
}

function slugify(value) {
  return String(value).toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'surface';
}

function emptySurfaceEvidence(surface) {
  return { surface, fullpage: null, focus: [], focusClip: null, motionClip: null, motionFrames: [] };
}

async function captureEvidenceWithRetry(surface, contextOptions, artifactsDir, errors) {
  let lastError = null;
  for (let attempt = 0; attempt < 2; attempt += 1) {
    try {
      return await captureEvidence(surface, contextOptions, artifactsDir, errors);
    } catch (error) {
      lastError = error;
      if (attempt === 0) errors.push(`capture for ${surface.id} failed once; retrying: ${error.message}`);
    }
  }
  errors.push(`capture for ${surface.id} failed after retry: ${lastError?.message || 'unknown error'}`);
  return emptySurfaceEvidence(surface);
}

async function captureEvidence(surface, contextOptions, artifactsDir, errors) {
  const media = emptySurfaceEvidence(surface);
  const prefix = `review-${surface.slug}`;

  // Full-page screenshot.
  let browser = null;
  let context = null;
  try {
    browser = await launchReviewBrowser();
    context = await browser.newContext(contextOptions);
    const page = await context.newPage();
    await page.goto(surface.url, { waitUntil: 'networkidle' });
    const fullpagePath = path.join(artifactsDir, `${prefix}-fullpage.png`);
    await page.screenshot({ path: fullpagePath, fullPage: true });
    media.fullpage = mediaEntry(surface, 'fullpage', 'screenshot', 'Full page as the AI reviewer saw it', fullpagePath);

    // Focus-state montage: tab through and screenshot the focused viewport.
    for (let i = 0; i < 6; i += 1) {
      await page.keyboard.press('Tab');
      const label = await page.evaluate(() => {
        const el = document.activeElement;
        if (!el || el === document.body) return 'body';
        return (el.getAttribute('aria-label') || el.textContent || el.tagName || '').trim().replace(/\s+/g, ' ').slice(0, 40);
      });
      if (label === 'body') break;
      const focusPath = path.join(artifactsDir, `${prefix}-focus-${i + 1}.png`);
      await page.screenshot({ path: focusPath });
      media.focus.push(mediaEntry(surface, 'focus', 'screenshot', `Keyboard focus on: ${label}`, focusPath));
    }
  } finally {
    if (context) await context.close().catch(() => {});
    if (browser) await closeReviewBrowser(browser);
  }

  // Focus clip: record tabbing through the page.
  media.focusClip = await recordClip(surface, contextOptions, artifactsDir, `${prefix}-focus-clip`, async (clipPage) => {
    for (let i = 0; i < 8; i += 1) {
      await clipPage.keyboard.press('Tab');
      await clipPage.waitForTimeout(220);
    }
  }, 'Keyboard focus moving through the page', errors);

  // Motion clip: let the page sit so any animation/auto-updating content plays.
  media.motionClip = await recordClip(surface, contextOptions, artifactsDir, `${prefix}-motion-clip`, async (clipPage) => {
    await clipPage.waitForTimeout(2600);
  }, 'The page over ~2.5s (motion / auto-updating content)', errors);

  // Motion montage: sample still frames over ~2.4s with motion ENABLED (as a
  // user with no reduced-motion preference would see it), so the vision model
  // can compare frames and actually judge motion / animation / auto-updating
  // content — a single static screenshot cannot show movement. These frames are
  // model-only; the report keeps the clip (better for a human than four stills).
  // Note: this DELIBERATELY overrides the manifest's reduced_motion (which the
  // motion clip above honors) — judging 2.2.x needs motion present to observe.
  let motionBrowser = null;
  let motionContext = null;
  try {
    motionBrowser = await launchReviewBrowser();
    motionContext = await motionBrowser.newContext({ ...contextOptions, reducedMotion: 'no-preference' });
    const motionPage = await motionContext.newPage();
    await motionPage.goto(surface.url, { waitUntil: 'networkidle' });
    for (let i = 0; i < 4; i += 1) {
      const framePath = path.join(artifactsDir, `${prefix}-motion-frame-${i + 1}.png`);
      await motionPage.screenshot({ path: framePath });
      media.motionFrames.push(mediaEntry(surface, 'motion_frame', 'screenshot', `Motion frame ${i + 1} (t≈${(i * 0.8).toFixed(1)}s)`, framePath, { modelOnly: true }));
      if (i < 3) await motionPage.waitForTimeout(800);
    }
  } catch (error) {
    errors.push(`motion montage for ${surface.id} failed: ${error.message}`);
  } finally {
    if (motionContext) await motionContext.close().catch(() => {});
    if (motionBrowser) await closeReviewBrowser(motionBrowser);
  }

  return media;
}

function mediaEntry(surface, role, kind, caption, absPath, extra = {}) {
  return {
    surface_id: surface.id,
    role,
    kind,
    caption: `${surface.id} (${surface.route}) - ${caption}`,
    absPath,
    ...extra,
  };
}

async function recordClip(surface, contextOptions, artifactsDir, name, actions, caption, errors) {
  let browser = null;
  let context = null;
  try {
    browser = await launchReviewBrowser();
    context = await browser.newContext({
      ...contextOptions,
      recordVideo: { dir: artifactsDir, size: contextOptions.viewport },
    });
    const page = await context.newPage();
    await page.goto(surface.url, { waitUntil: 'networkidle' });
    await actions(page);
    const video = page.video();
    await context.close();
    context = null;
    if (!video) return null;
    const src = await video.path();
    const dest = path.join(artifactsDir, `${name}.webm`);
    await fs.copyFile(src, dest);
    await fs.rm(src, { force: true }).catch(() => {});
    return mediaEntry(surface, 'clip', 'clip', caption, dest);
  } catch (error) {
    errors.push(`clip ${name} failed: ${error.message}`);
    return null;
  } finally {
    if (context) await context.close().catch(() => {});
    if (browser) await closeReviewBrowser(browser);
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

function recordSurfaceAssessment(results, { item, surface, verdict, media }) {
  const rawVerdict = (verdict?.verdict || '').toLowerCase();
  const settled = rawVerdict === 'pass' || rawVerdict === 'fail';
  const result = {
    surface,
    verdict: settled ? rawVerdict : 'inconclusive',
    confidence: settled ? (verdict?.confidence || 'low') : 'not_observed',
    rationale: verdict?.rationale || `Agentic review did not return a verdict for ${surface.id}; the captured evidence is attached for human review.`,
    reviewer_guidance: verdict?.reviewer_guidance || `Review the attached ${surface.id} evidence manually against this criterion.`,
    media,
  };
  if (!results.has(item.obligation)) results.set(item.obligation, []);
  results.get(item.obligation).push(result);
}

function finalizeAssessments(criteria, results, artifactsDir) {
  const seen = new Set();
  const assessments = [];
  for (const item of criteria) {
    if (seen.has(item.obligation)) continue;
    seen.add(item.obligation);
    assessments.push(finalizeAssessment(item.obligation, results.get(item.obligation) || [], artifactsDir));
  }
  return assessments;
}

function finalizeAssessment(obligation, surfaceResults, artifactsDir) {
  const failing = surfaceResults.find((result) => result.verdict === 'fail');
  const inconclusive = surfaceResults.find((result) => result.verdict !== 'pass');
  const reviewedSurfaces = surfaceResults.map((result) => result.surface.id).join(', ') || 'none';
  if (failing) {
    return {
      obligation,
      verdict: 'fail',
      confidence: failing.confidence,
      rationale: `Surface ${failing.surface.id} failed: ${failing.rationale}`,
      reviewer_guidance: failing.reviewer_guidance,
      media: assessmentMedia(surfaceResults, artifactsDir),
    };
  }
  if (inconclusive || surfaceResults.length === 0) {
    return {
      obligation,
      verdict: 'inconclusive',
      confidence: 'not_observed',
      rationale: inconclusive
        ? `Surface ${inconclusive.surface.id} was inconclusive: ${inconclusive.rationale}`
        : 'Agentic review did not return a surface verdict for this criterion; captured evidence is attached where available.',
      reviewer_guidance: inconclusive?.reviewer_guidance || 'Review the attached surface evidence manually against this criterion.',
      media: assessmentMedia(surfaceResults, artifactsDir),
    };
  }
  return {
    obligation,
    verdict: 'pass',
    confidence: weakestConfidence(surfaceResults.map((result) => result.confidence)),
    rationale: `All reviewed surfaces passed (${reviewedSurfaces}).`,
    reviewer_guidance: surfaceResults[0]?.reviewer_guidance || 'Spot-check the attached surface evidence manually.',
    media: assessmentMedia(surfaceResults, artifactsDir),
  };
}

function assessmentMedia(surfaceResults, artifactsDir) {
  const seen = new Set();
  const out = [];
  for (const result of surfaceResults) {
    for (const entry of result.media) {
      if (!entry?.absPath || seen.has(entry.absPath)) continue;
      seen.add(entry.absPath);
      out.push({
        kind: entry.kind,
        caption: entry.caption,
        path: path.relative(artifactsDir, entry.absPath).split(path.sep).join('/'),
      });
    }
  }
  return out;
}

function weakestConfidence(values) {
  if (values.includes('low')) return 'low';
  if (values.includes('medium')) return 'medium';
  if (values.includes('high')) return 'high';
  return 'not_observed';
}

function evaluatePrecisionGate(gateRequest, assessmentResults) {
  const labels = Array.isArray(gateRequest?.labels) ? gateRequest.labels.map(normalizePrecisionLabel).filter(Boolean) : [];
  if (labels.length === 0) return null;
  const falsePositiveLabels = [];
  const missingLabels = [];
  for (const label of labels) {
    const result = (assessmentResults.get(label.obligation) || []).find((entry) => entry.surface.id === label.surface_id);
    if (!result || result.verdict === 'inconclusive') {
      missingLabels.push(label);
      continue;
    }
    if (label.expected === 'pass' && result.verdict === 'fail') {
      falsePositiveLabels.push(label);
    }
  }
  const expectedPassCases = labels.filter((label) => label.expected === 'pass').length;
  const status = falsePositiveLabels.length === 0 && missingLabels.length === 0 ? 'pass' : 'fail';
  return {
    schema: 'allie.agentic.precision_gate.v0',
    status,
    labeled_cases: labels.length,
    expected_pass_cases: expectedPassCases,
    fail_false_positives: falsePositiveLabels.length,
    missing_labels: missingLabels.length,
    false_positive_labels: falsePositiveLabels,
  };
}

function normalizePrecisionLabel(label) {
  const surfaceId = String(label?.surface_id || '').trim();
  const obligation = String(label?.obligation || '').trim();
  const expected = String(label?.expected || '').trim().toLowerCase();
  if (!surfaceId || !obligation || !['pass', 'fail'].includes(expected)) return null;
  return { surface_id: surfaceId, obligation, expected };
}

// --- model boundary ---------------------------------------------------------

async function assessGroupWithReviewLoop(model, apiKey, group, groupMedia, context) {
  let media = groupMedia;
  let finalResult = { verdicts: {}, usage: {}, actions: [] };
  const usage = { prompt_tokens: 0, completion_tokens: 0 };
  let calls = 0;

  for (let attempt = 0; attempt < 2 && calls < context.budget; attempt += 1) {
    let result;
    try {
      result = await assessGroup(model, apiKey, { ...group, rejudge_attempt: attempt }, media, context);
    } catch (error) {
      calls += error.modelCalls || 0;
      error.modelCalls = calls;
      throw error;
    }
    calls += result.calls;
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

async function captureReviewActionMedia({ surface, contextOptions, artifactsDir, errors, groupKind, attempt, actions }) {
  let browser = null;
  let context = null;
  try {
    browser = await launchReviewBrowser();
    context = await browser.newContext(contextOptions);
    const page = await context.newPage();
    await page.goto(surface.url, { waitUntil: 'networkidle' });
    for (const action of actions) {
      if (action.type === 'press_key') {
        await page.keyboard.press(action.key);
        await page.waitForTimeout(180);
      } else if (action.type === 'wait_ms') {
        await page.waitForTimeout(action.ms);
      }
    }
    const actionPath = path.join(artifactsDir, `review-${surface.slug}-action-${groupKind}-${attempt}.png`);
    await page.screenshot({ path: actionPath });
    return [mediaEntry(surface, 'review_action', 'screenshot', `Review action ${attempt}: ${actionSummary(actions)}`, actionPath)];
  } catch (error) {
    errors.push(`review action for ${groupKind} on ${surface.id} failed: ${error.message}`);
    return [];
  } finally {
    if (context) await context.close().catch(() => {});
    if (browser) await closeReviewBrowser(browser);
  }
}

async function launchReviewBrowser() {
  return chromium.launch(CHROMIUM_LAUNCH_OPTIONS);
}

async function closeReviewBrowser(browser) {
  await Promise.race([
    browser.close().catch(() => {}),
    new Promise((resolve) => setTimeout(resolve, 1000)),
  ]);
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

async function assessGroup(model, apiKey, group, groupMedia, context) {
  const imageMedia = modelImageMedia(groupMedia);
  const videoMedia = groupMedia.filter(isVideoMedia).slice(0, 2);
  const content = [{ type: 'text', text: buildPrompt(group, [...imageMedia, ...videoMedia], context.surface) }];
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
  const { json, calls } = await fetchModelJsonWithRetry(base, apiKey, body, context.budget);
  const text = json.choices?.[0]?.message?.content || '';
  const parsed = parseModelJson(text);
  const verdicts = {};
  for (const entry of parsed?.assessments || []) {
    if (entry.obligation) verdicts[entry.obligation] = entry;
  }
  if (Object.keys(verdicts).length === 0) {
    context.errors.push(`model returned no parseable assessments for ${group.kind} on ${context.surface.id}`);
  }
  return { verdicts, actions: parsed?.actions || [], usage: json.usage, calls };
}

async function fetchModelJsonWithRetry(base, apiKey, body, budget) {
  const maxAttempts = Math.max(1, Math.min(1 + MAX_MODEL_RETRIES, budget));
  let calls = 0;
  let lastError = null;
  for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
    calls += 1;
    try {
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
        const error = new Error(`HTTP ${res.status}: ${text.slice(0, 200)}`);
        if (isTransientModelFailure(res.status) && attempt + 1 < maxAttempts) {
          lastError = error;
          continue;
        }
        error.modelCalls = calls;
        throw error;
      }
      return { json: await res.json(), calls };
    } catch (error) {
      if (error.modelCalls) throw error;
      lastError = error;
      if (attempt + 1 < maxAttempts) continue;
      error.modelCalls = calls;
      throw error;
    }
  }
  const error = lastError || new Error('model call failed without response');
  error.modelCalls = calls;
  throw error;
}

function isTransientModelFailure(status) {
  return status === 408 || status === 409 || status === 425 || status === 429 || status >= 500;
}

function modelImageMedia(groupMedia) {
  const screenshots = groupMedia.filter((entry) => entry.kind === 'screenshot');
  const actionScreenshots = screenshots.filter((entry) => entry.role === 'review_action');
  const baselineScreenshots = screenshots.filter((entry) => entry.role !== 'review_action');
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

function buildPrompt(group, media = [], surface = null) {
  const list = group.items
    .map((item) => `- ${item.obligation} | ${item.num} ${item.handle} (Level ${item.level}, ${item.principle})`)
    .join('\n');
  const mediaList = media
    .map((entry, index) => `- media ${index + 1}: ${entry.kind} | ${entry.caption}`)
    .join('\n');
  const allowedKeys = Array.from(ALLOWED_REVIEW_KEYS).join('|');
  return [
    'You are an expert WCAG 2.2 AA accessibility auditor. You are doing the job a trained human reviewer does: looking at the captured visual evidence and rendering a DEFINITIVE judgment for each success criterion.',
    surface ? `Surface: ${surface.id} | route ${surface.route} | title ${surface.title}` : 'Surface: target',
    `Focus area for this batch: ${group.guidance}`,
    'For each criterion, make the same call a human expert would make from this evidence:',
    '- verdict: "pass" or "fail". You MUST commit to one. Use "inconclusive" ONLY when even an expert genuinely cannot judge from any amount of looking (e.g. a precise color-contrast ratio that requires a measurement tool). Do not hide behind "inconclusive" to avoid deciding.',
    '- confidence: "high" | "medium" | "low" — be honest. Low confidence is fine and is exactly how we mark a judgment call; it does NOT mean refuse to decide.',
    '- rationale: one to two sentences grounded in what you actually see.',
    '- reviewer_guidance: the exact thing a human should do to confirm or refute your verdict.',
    `If a small browser observation would let you make a better judgment, you may request at most three review actions before the final verdict. Allowed action shapes are {"type":"press_key","key":"${allowedKeys}","reason":"..."} and {"type":"wait_ms","ms":100-2000}. Do not request clicks, typing, form submission, navigation, activation keys, or mutation.`,
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
