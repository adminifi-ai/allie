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
const server = http.createServer((request, response) => {
  let body = '';
  request.setEncoding('utf8');
  request.on('data', (chunk) => {
    body += chunk;
  });
  request.on('end', () => {
    capturedRequests.push(JSON.parse(body));
    response.setHeader('Content-Type', 'application/json');
    response.end(JSON.stringify(fakeOpenRouterResponse()));
  });
});

await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
try {
  const requestPath = path.join(work, 'model-request.json');
  const responsePath = path.join(work, 'model-response.json');
  await fs.writeFile(requestPath, `${JSON.stringify(agenticRequest(server.address().port), null, 2)}\n`);

  const code = await runAgenticWorker(requestPath, responsePath);
  if (code !== 0) {
    throw new Error(`agentic worker exited ${code}`);
  }

  assertFakeProviderSawVideo();
  await assertWorkerResponse(responsePath);
  console.log('agentic model payload ok: fake OpenRouter request included screenshot and video_url media');
} finally {
  await new Promise((resolve) => server.close(resolve));
}

function agenticRequest(port) {
  return {
    schema: 'allie.agentic.request.v0',
    target: { fixture_dir: path.join(repo, 'fixtures/login') },
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
      max_calls: 1,
    },
    artifacts_dir: path.join(work, 'model-artifacts'),
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

function fakeOpenRouterResponse() {
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
                rationale: 'The fake model inspected the supplied focus media.',
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

function assertFakeProviderSawVideo() {
  if (capturedRequests.length !== 1) {
    throw new Error(`expected one fake model request, captured ${capturedRequests.length}`);
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
}

async function assertWorkerResponse(responsePath) {
  const workerResponse = JSON.parse(await fs.readFile(responsePath, 'utf8'));
  if (workerResponse.status !== 'ok' || workerResponse.calls !== 1) {
    throw new Error(`expected successful fake model call, got status=${workerResponse.status} calls=${workerResponse.calls}`);
  }
  if (!workerResponse.assessments[0].media.some((entry) => entry.kind === 'clip')) {
    throw new Error('agentic response did not keep the captured clip attached for report review');
  }
}
