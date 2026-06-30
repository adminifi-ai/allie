#!/usr/bin/env node
import fs from 'node:fs/promises';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';

const repo = process.cwd();
const work = await fs.mkdtemp(path.join(os.tmpdir(), 'allie-agentic-precision-'));
try {
  await runScenario({
    name: 'false-positive',
    labels: [
      label('home', 'pass'),
      label('settings', 'pass'),
    ],
    expectedGate: 'fail',
    expectedFalsePositives: 1,
    expectedStatus: 'degraded',
  });
  await runScenario({
    name: 'zero-fp',
    labels: [
      label('home', 'pass'),
      label('settings', 'fail'),
    ],
    expectedGate: 'pass',
    expectedFalsePositives: 0,
    expectedStatus: 'ok',
  });
  console.log('agentic fail precision smoke passed: labeled false positives gate FAIL promotion');
} finally {
  await fs.rm(work, { recursive: true, force: true });
}

async function runScenario({ name, labels, expectedGate, expectedFalsePositives, expectedStatus }) {
  const server = fakeOpenRouterServer();
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  try {
    const requestPath = path.join(work, `${name}-request.json`);
    const responsePath = path.join(work, `${name}-response.json`);
    await fs.writeFile(requestPath, `${JSON.stringify(agenticRequest(server.address().port, labels, name), null, 2)}\n`);
    const code = await runAgenticWorker(requestPath, responsePath);
    if (code !== 0) throw new Error(`${name}: agentic worker exited ${code}`);
    const response = JSON.parse(await fs.readFile(responsePath, 'utf8'));
    const gate = response.precision_gate;
    if (!gate) throw new Error(`${name}: missing precision_gate in worker response`);
    if (gate.status !== expectedGate) {
      throw new Error(`${name}: expected precision gate ${expectedGate}, got ${gate.status}`);
    }
    if (gate.fail_false_positives !== expectedFalsePositives) {
      throw new Error(`${name}: expected ${expectedFalsePositives} false positive(s), got ${gate.fail_false_positives}`);
    }
    if (response.status !== expectedStatus) {
      throw new Error(`${name}: expected response status ${expectedStatus}, got ${response.status}`);
    }
    if (response.assessments[0]?.verdict !== 'fail') {
      throw new Error(`${name}: expected aggregate assessment to remain fail for gate testing`);
    }
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
}

function label(surfaceId, expected) {
  return {
    surface_id: surfaceId,
    obligation: 'wcag22-aa:2.4.7-focus-visible',
    expected,
  };
}

function agenticRequest(port, labels, name) {
  return {
    schema: 'allie.agentic.request.v0',
    target: { fixture_dir: path.join(repo, 'fixtures/workbench') },
    browser: {
      viewport: { width: 1024, height: 768 },
      color_scheme: 'light',
      reduced_motion: 'reduce',
      locale: 'en-US',
    },
    model: {
      provider: 'openrouter',
      model: `fake-precision-${name}`,
      api_key_env: 'ALLIE_AGENTIC_FAKE_KEY',
      base_url: `http://127.0.0.1:${port}`,
      max_calls: 4,
    },
    artifacts_dir: path.join(work, `${name}-artifacts`),
    surfaces: [
      { id: 'home', route: '/', url: '/' },
      { id: 'settings', route: '/settings.html', url: '/settings.html' },
    ],
    precision_gate: { labels },
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

function fakeOpenRouterServer() {
  return http.createServer((request, response) => {
    let body = '';
    request.setEncoding('utf8');
    request.on('data', (chunk) => {
      body += chunk;
    });
    request.on('end', () => {
      const prompt = promptText(JSON.parse(body));
      const verdict = prompt.includes('Surface: settings') ? 'fail' : 'pass';
      response.setHeader('Content-Type', 'application/json');
      response.end(JSON.stringify({
        choices: [
          {
            message: {
              content: JSON.stringify({
                assessments: [
                  {
                    obligation: 'wcag22-aa:2.4.7-focus-visible',
                    verdict,
                    confidence: 'high',
                    rationale: `Synthetic ${verdict} for ${prompt.includes('Surface: settings') ? 'settings' : 'home'}.`,
                    reviewer_guidance: 'Synthetic precision smoke response.',
                  },
                ],
              }),
            },
          },
        ],
        usage: { prompt_tokens: 1, completion_tokens: 1 },
      }));
    });
  });
}

function promptText(body) {
  return body.messages?.[0]?.content?.find((part) => part.type === 'text')?.text || '';
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
