#!/usr/bin/env node
import fs from 'node:fs/promises';
import http from 'node:http';
import path from 'node:path';
import { spawn } from 'node:child_process';

const [work, repo] = process.argv.slice(2);
if (!work || !repo) {
  throw new Error('usage: agentic-video-payload-smoke.mjs <work-dir> <repo-root>');
}

const capturedRequests = [];
let settingsTransientReturned = false;
const server = http.createServer((request, response) => {
  let body = '';
  request.setEncoding('utf8');
  request.on('data', (chunk) => {
    body += chunk;
  });
  request.on('end', () => {
    const parsed = JSON.parse(body);
    capturedRequests.push(parsed);
    if (isSettingsPrompt(parsed) && !settingsTransientReturned) {
      settingsTransientReturned = true;
      response.writeHead(503, { 'Content-Type': 'text/plain' });
      response.end('synthetic transient model outage');
      return;
    }
    response.setHeader('Content-Type', 'application/json');
    response.end(JSON.stringify(fakeOpenRouterResponse(parsed)));
  });
});

await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
try {
  await assertMissingAndUnsupportedRedactionFailClosed();
  const requestPath = path.join(work, 'model-request.json');
  const responsePath = path.join(work, 'model-response.json');
  await fs.writeFile(requestPath, `${JSON.stringify(agenticRequest(server.address().port), null, 2)}\n`);

  const code = await runAgenticWorker(requestPath, responsePath);
  if (code !== 0) {
    throw new Error(`agentic worker exited ${code}`);
  }

  assertFakeProviderSawSurfaceFanout();
  await assertWorkerResponse(responsePath);
  console.log('agentic model payload ok: fake OpenRouter saw multi-surface video media, retry, and observe-act-rejudge');
} finally {
  await new Promise((resolve) => server.close(resolve));
}

function agenticRequest(port) {
  return {
    schema: 'allie.agentic.request.v0',
    prompt_version: 'allie.agentic.wcag-review.v1',
    target: { fixture_dir: path.join(repo, 'fixtures/workbench') },
    browser: {
      viewport: { width: 1024, height: 768 },
      color_scheme: 'light',
      reduced_motion: 'reduce',
      locale: 'en-US',
    },
    model: {
      provider: 'openrouter',
      model: 'fake-video-capable-model',
      api_key_env: 'ALLIE_AGENTIC_FAKE_KEY',
      base_url: `http://127.0.0.1:${port}`,
      max_calls: 5,
      redaction: 'none',
    },
    artifacts_dir: path.join(work, 'model-artifacts'),
    surfaces: [
      { id: 'home', route: '/', url: '/' },
      { id: 'settings', route: '/settings.html', url: '/settings.html' },
    ],
    criteria: [
      {
        obligation: 'wcag22-aa:2.4.7-focus-visible',
        num: '2.4.7',
        handle: 'Focus Visible',
        level: 'AA',
        principle: 'Operable',
      },
    ],
  };
}

function fakeOpenRouterResponse(body) {
  const prompt = promptText(body);
  if (prompt.includes('Surface: home') && !prompt.includes('Review action')) {
    return {
      choices: [
        {
          message: {
            content: JSON.stringify({
              actions: [
                {
                  type: 'press_key',
                  key: 'Tab',
                  reason: 'Inspect the next keyboard focus state before deciding.',
                },
              ],
              assessments: [
                {
                  obligation: 'wcag22-aa:2.4.7-focus-visible',
                  verdict: 'inconclusive',
                  confidence: 'low',
                  rationale: 'The initial media is not enough for the fake model.',
                  reviewer_guidance: 'Capture the next focus state and re-judge.',
                },
              ],
            }),
          },
        },
      ],
      usage: { prompt_tokens: 1, completion_tokens: 1 },
    };
  }

  if (prompt.includes('Surface: settings')) {
    return {
      choices: [
        {
          message: {
            content: JSON.stringify({
              assessments: [
                {
                  obligation: 'wcag22-aa:2.4.7-focus-visible',
                  verdict: 'fail',
                  confidence: 'medium',
                  rationale: 'The fake settings surface failed the supplied focus check.',
                  reviewer_guidance: 'Inspect the settings focus evidence manually.',
                },
              ],
            }),
          },
        },
      ],
      usage: { prompt_tokens: 1, completion_tokens: 1 },
    };
  }

  return {
    choices: [
      {
        message: {
          content: JSON.stringify({
            assessments: [
              {
                obligation: 'wcag22-aa:2.4.7-focus-visible',
                verdict: 'pass',
                confidence: 'medium',
                rationale: 'The fake model inspected the supplied home focus media.',
                reviewer_guidance: 'Confirm the attached focus walkthrough manually.',
              },
            ],
          }),
        },
      },
    ],
    usage: { prompt_tokens: 1, completion_tokens: 1 },
  };
}

function runAgenticWorker(requestPath, responsePath) {
  return new Promise((resolve) => {
    const child = spawn(process.execPath, [
      path.join(repo, 'workers/agentic/review.mjs'),
      '--request',
      requestPath,
      '--response',
      responsePath,
    ], {
      cwd: repo,
      env: { ...process.env, ALLIE_AGENTIC_FAKE_KEY: 'test-key' },
      stdio: ['ignore', 'ignore', 'pipe'],
    });
    let stderr = '';
    child.stderr.on('data', (chunk) => {
      stderr += chunk;
    });
    child.on('close', (status) => {
      if (status !== 0) process.stderr.write(stderr);
      resolve(status);
    });
  });
}

function assertFakeProviderSawSurfaceFanout() {
  if (capturedRequests.length !== 4) {
    throw new Error(`expected home rejudge plus settings retry to make four fake model requests, captured ${capturedRequests.length}`);
  }
  const content = capturedRequests[0].messages?.[0]?.content;
  if (!Array.isArray(content)) {
    throw new Error('fake model request did not contain chat content parts');
  }
  if (!content.some((part) => part.type === 'image_url' && part.image_url?.url?.startsWith('data:image/png;base64,'))) {
    throw new Error('fake model request did not include screenshot image_url media');
  }
  if (!content.some((part) => part.type === 'video_url' && part.video_url?.url?.startsWith('data:video/webm;base64,'))) {
    throw new Error('fake model request did not include video_url walkthrough media');
  }
  const firstPrompt = promptText(capturedRequests[0]);
  if (!firstPrompt.includes('Surface: home')) {
    throw new Error('first fake model request did not identify the home surface');
  }
  if (firstPrompt.includes('Enter|Space')) {
    throw new Error('review-action prompt still permits activation keys that can submit or mutate app state');
  }
  const secondContent = capturedRequests[1].messages?.[0]?.content;
  if (!Array.isArray(secondContent)) {
    throw new Error('rejudge request did not contain chat content parts');
  }
  const secondPrompt = promptText(capturedRequests[1]);
  if (!secondPrompt.includes('Review action')) {
    throw new Error('rejudge request did not name the action-captured screenshot media');
  }
  const settingsPrompts = capturedRequests.map(promptText).filter((prompt) => prompt.includes('Surface: settings'));
  if (settingsPrompts.length !== 2 || !settingsTransientReturned) {
    throw new Error('settings surface was not retried after the synthetic transient model outage');
  }
}

async function assertWorkerResponse(responsePath) {
  const workerResponse = JSON.parse(await fs.readFile(responsePath, 'utf8'));
  if (workerResponse.status !== 'ok' || workerResponse.calls !== 4) {
    throw new Error(`expected successful fake model call, got status=${workerResponse.status} calls=${workerResponse.calls}`);
  }
  if (workerResponse.assessments[0].verdict !== 'fail') {
    throw new Error(`expected final fanout verdict to fail when one surface fails, got ${workerResponse.assessments[0].verdict}`);
  }
  if (!workerResponse.assessments[0].media.some((entry) => entry.kind === 'clip')) {
    throw new Error('agentic response did not keep the captured clip attached for report review');
  }
  if (!workerResponse.assessments[0].media.some((entry) => entry.caption.includes('Review action'))) {
    throw new Error('agentic response did not include the action-captured rejudge screenshot');
  }
  if (!workerResponse.assessments[0].media.some((entry) => entry.caption.includes('settings'))) {
    throw new Error('agentic response did not include settings-surface media');
  }
  const receipt = workerResponse.redaction_receipt;
  if (receipt?.schema !== 'allie.model-redaction-receipt.v0' || receipt.profile !== 'none' || receipt.status !== 'not_applied') {
    throw new Error(`fake-provider response did not retain a truthful not_applied receipt: ${JSON.stringify(receipt)}`);
  }
}

async function assertMissingAndUnsupportedRedactionFailClosed() {
  for (const [label, redaction] of [['missing', undefined], ['unsupported', 'blur-v1']]) {
    const request = agenticRequest(server.address().port);
    if (redaction === undefined) delete request.model.redaction;
    else request.model.redaction = redaction;
    const requestPath = path.join(work, `${label}-redaction-request.json`);
    const responsePath = path.join(work, `${label}-redaction-response.json`);
    request.artifacts_dir = path.join(work, `${label}-redaction-artifacts`);
    await fs.writeFile(requestPath, `${JSON.stringify(request, null, 2)}\n`);

    const before = capturedRequests.length;
    const code = await runAgenticWorker(requestPath, responsePath);
    if (code !== 1 || capturedRequests.length !== before) {
      throw new Error(`${label} redaction mode did not fail before provider transmission`);
    }
    const response = JSON.parse(await fs.readFile(responsePath, 'utf8'));
    const receipt = response.redaction_receipt;
    if (receipt?.schema !== 'allie.model-redaction-receipt.v0' || receipt.profile !== 'none' || receipt.status !== 'not_sent') {
      throw new Error(`${label} redaction refusal did not carry a truthful not_sent receipt: ${JSON.stringify(receipt)}`);
    }
    if (await pathExists(request.artifacts_dir)) {
      throw new Error(`${label} redaction mode created capture artifacts before refusal`);
    }
  }
}

async function pathExists(target) {
  try {
    await fs.access(target);
    return true;
  } catch {
    return false;
  }
}

function promptText(body) {
  return body.messages?.[0]?.content?.find((part) => part.type === 'text')?.text || '';
}

function isSettingsPrompt(body) {
  return promptText(body).includes('Surface: settings');
}
