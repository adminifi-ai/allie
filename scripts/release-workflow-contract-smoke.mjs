import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import TOML from '@iarna/toml';
import { parseDocument, stringify as stringifyYaml } from 'yaml';

const RELEASE_PATH = '.github/workflows/release.yml';
const CI_PATH = '.github/workflows/ci.yml';
const AUDIT_PATH = '.cargo/audit.toml';
const WAIVER_PATH = '.cargo/audit-waivers.toml';
const README_PATH = 'README.md';
const ACTION_SHA = /^[^@\s]+@[0-9a-f]{40}$/;
const COSIGN_INSTALLER = /^sigstore\/cosign-installer@[0-9a-f]{40}$/;
const SIGN_COMMAND = 'cosign sign-blob --yes --bundle dist/allie-linux-x64.tar.gz.sigstore.json dist/allie-linux-x64.tar.gz';
const EXPECTED_ASSETS = [
  'SHA256SUMS',
  'allie-linux-x64.tar.gz',
  'allie-linux-x64.tar.gz.sigstore.json',
];

function fail(message) {
  throw new Error(message);
}

function parseYaml(text, label) {
  const document = parseDocument(text);
  if (document.errors.length > 0) fail(`${label} is not valid YAML: ${document.errors[0].message}`);
  return document.toJS();
}

function exactKeys(value, expected, label) {
  const actual = Object.keys(value || {}).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    fail(`${label} keys must be exactly [${wanted.join(', ')}], got [${actual.join(', ')}]`);
  }
}

function actionStep(steps, owner) {
  return steps.find((step) => String(step.uses || '').startsWith(`${owner}@`));
}

function lines(value) {
  return String(value || '').split('\n').map((line) => line.trim()).filter(Boolean);
}

function validateReleaseWorkflow(text) {
  const workflow = parseYaml(text, 'release workflow');
  exactKeys(workflow.permissions, [], 'global release permissions');
  const triggers = workflow.on || {};
  if (!triggers.push?.tags?.includes('v*') || Object.hasOwn(triggers, 'workflow_dispatch')) {
    fail('release workflow must accept only version tag pushes');
  }
  const build = workflow.jobs?.['build-release'];
  const publish = workflow.jobs?.['sign-and-publish'];
  if (!build || !publish) fail('release workflow must split build-release from sign-and-publish');
  exactKeys(build.permissions, ['contents'], 'build-release permissions');
  if (build.permissions.contents !== 'read') fail('build-release may only read repository contents');
  exactKeys(publish.permissions, ['contents', 'id-token'], 'sign-and-publish permissions');
  if (publish.permissions.contents !== 'write' || publish.permissions['id-token'] !== 'write') {
    fail('sign-and-publish must have only contents:write and id-token:write');
  }
  if (publish.needs !== 'build-release') fail('sign-and-publish must consume only build-release');

  const allSteps = [...(build.steps || []), ...(publish.steps || [])];
  for (const step of allSteps.filter((item) => item.uses)) {
    if (!ACTION_SHA.test(step.uses)) fail(`release action is not pinned to a full SHA: ${step.uses}`);
    if (String(step.uses).startsWith('actions/checkout@') && step.with?.['persist-credentials'] !== false) {
      fail('release checkout must set persist-credentials: false');
    }
  }
  if ((publish.steps || []).some((step) => String(step.uses || '').startsWith('actions/checkout@'))) {
    fail('privileged sign-and-publish must not checkout repository content');
  }
  const privilegedRuns = (publish.steps || []).map((step) => String(step.run || '')).join('\n');
  if (/\b(cargo|npm|playwright|package-release|release-checksums)\b/.test(privilegedRuns)) {
    fail('privileged sign-and-publish must not build, test, or package');
  }
  for (const step of allSteps) {
    if (step['continue-on-error'] === true) fail('release steps may not continue on error');
    if (/\|\|\s*(true|:)(\s|$)/m.test(String(step.run || ''))) {
      fail('release shell may not bypass failure with || true or || :');
    }
  }

  const uploadArtifact = actionStep(build.steps || [], 'actions/upload-artifact');
  if (!uploadArtifact || uploadArtifact.with?.name !== 'unsigned-release-bundle') {
    fail('build-release must upload the named unsigned-release-bundle artifact');
  }
  const staged = lines(uploadArtifact.with.path);
  const expectedStaged = ['dist/SHA256SUMS', 'dist/allie-linux-x64.tar.gz'];
  if (JSON.stringify(staged.sort()) !== JSON.stringify(expectedStaged)) {
    fail(`unsigned artifact paths must be exact, got ${staged.join(', ')}`);
  }
  if (uploadArtifact.with['if-no-files-found'] !== 'error') fail('missing release inputs must fail');

  const downloadArtifact = actionStep(publish.steps || [], 'actions/download-artifact');
  if (downloadArtifact?.with?.name !== 'unsigned-release-bundle' || downloadArtifact.with.path !== 'dist') {
    fail('sign-and-publish must download the exact unsigned artifact into dist');
  }
  const cosignInstallers = (publish.steps || []).filter((step) => /\/cosign-installer@/.test(String(step.uses || '')));
  if (cosignInstallers.length !== 1 || !COSIGN_INSTALLER.test(String(cosignInstallers[0]?.uses || ''))) {
    fail('sign-and-publish must use exactly the official full-SHA-pinned sigstore/cosign-installer action');
  }
  const sign = (publish.steps || []).find((step) => step.name === 'Sign release bundle with GitHub OIDC');
  const signRun = String(sign?.run || '').trim();
  if (signRun !== SIGN_COMMAND) {
    fail('signing must be the exact fail-closed archive and Sigstore bundle command');
  }

  const release = (publish.steps || []).find((step) => step.name === 'Publish only after exact asset readback');
  const run = String(release?.run || '');
  const draftAt = run.indexOf('gh api --method POST "repos/$GITHUB_REPOSITORY/releases"');
  const uploadAt = run.indexOf('gh release upload "$tag"');
  const readbackAt = run.indexOf('"repos/$GITHUB_REPOSITORY/releases/$release_id" --jq');
  const publishAt = run.indexOf('gh api --method PATCH "repos/$GITHUB_REPOSITORY/releases/$release_id" -F draft=false');
  if (!(draftAt >= 0 && uploadAt > draftAt && readbackAt > uploadAt && publishAt > readbackAt)) {
    fail('release order must be draft, upload, API asset readback, then publish');
  }
  if (!run.includes('-F generate_release_notes=true') || /\s-f body=/.test(run)) {
    fail('release publication must use GitHub-generated notes');
  }
  const uploadLine = run.split('\n').find((line) => line.trimStart().startsWith('gh release upload '));
  if (!uploadLine?.includes('--repo "$GITHUB_REPOSITORY"')) {
    fail('gh release upload must use explicit repository context without a checkout');
  }
  if (!run.includes('tag="$GITHUB_REF_NAME"') ||
      !run.includes('[[ "$tag" =~ ^v0\\.[0-9]+\\.[0-9]+$ ]]') ||
      !run.includes('"repos/$GITHUB_REPOSITORY/commits/$tag" --jq .sha') ||
      !run.includes('[ "$tag_commit" != "$GITHUB_SHA" ]')) {
    fail('release publication must validate and bind the pushed tag to the workflow commit');
  }
  if (!run.includes('set -euo pipefail') || !run.includes('trap cleanup EXIT') ||
      !run.includes('gh api --method DELETE "repos/$GITHUB_REPOSITORY/releases/$release_id"')) {
    fail('draft publication must fail closed and delete the exact failed draft');
  }
  if (run.includes('--clobber') || run.includes('dist/*')) fail('release upload may not clobber or glob assets');
  const uploadBlock = run.slice(uploadAt, run.indexOf('\n\n', uploadAt));
  const uploaded = [...uploadBlock.matchAll(/dist\/([A-Za-z0-9._-]+)/g)].map((match) => match[1]).sort();
  if (JSON.stringify(uploaded) !== JSON.stringify([...EXPECTED_ASSETS].sort())) {
    fail(`release upload assets must be exact, got ${uploaded.join(', ')}`);
  }
  const expectedStart = run.indexOf('expected_assets=');
  const expectedEnd = run.indexOf('\nif [ "$actual_assets"', expectedStart);
  if (expectedStart < 0 || expectedEnd < 0) fail('release must declare exact expected assets before comparison');
  const expectedBlock = run.slice(expectedStart, expectedEnd);
  for (const asset of EXPECTED_ASSETS) {
    if (!expectedBlock.includes(asset)) fail(`API readback expectation omitted ${asset}`);
  }
  if (run.slice(0, readbackAt).includes('-F draft=false')) {
    fail('release may not become public before exact API readback');
  }
}


function validateCiWorkflow(text) {
  const workflow = parseYaml(text, 'CI workflow');
  const audit = workflow.jobs?.['supply-chain-audit'];
  if (!audit || audit === workflow.jobs?.verify) fail('dependency audits must run in a distinct CI job');
  exactKeys(audit.permissions, ['contents'], 'supply-chain-audit permissions');
  if (audit.permissions.contents !== 'read') fail('supply-chain audit may only read contents');
  const commands = (audit.steps || []).map((step) => String(step.run || '')).filter(Boolean);
  if (!commands.includes('cargo audit')) fail('CI must run cargo audit without hidden ignore flags');
  if (!commands.includes('npm audit --audit-level=high')) fail('CI must fail on high npm advisories');
  for (const step of audit.steps || []) {
    if (step['continue-on-error'] === true || /\|\|\s*(true|:)/.test(String(step.run || ''))) {
      fail('supply-chain audit may not bypass failure');
    }
  }
}

function parseToml(text, label) {
  try {
    return TOML.parse(text);
  } catch (error) {
    fail(`${label} is malformed TOML: ${error.message}`);
  }
}

function validateAuditPolicy(auditText, waiverText, today = new Date()) {
  const audit = parseToml(auditText, 'cargo-audit policy');
  const policy = parseToml(waiverText, 'audit waiver policy');
  const ignored = audit.advisories?.ignore;
  if (!Array.isArray(ignored)) fail('cargo-audit policy must declare advisories.ignore');
  if (policy.schema !== 1 || !Array.isArray(policy.waiver)) fail('audit waiver policy must declare schema=1 and waiver=[]');
  const records = new Map();
  for (const waiver of policy.waiver) {
    exactKeys(waiver, ['advisory', 'expiry', 'owner', 'rationale', 'removal', 'tracking_ref'], 'audit waiver');
    for (const field of ['advisory', 'owner', 'rationale', 'removal', 'tracking_ref']) {
      if (typeof waiver[field] !== 'string' || waiver[field].trim() === '') fail(`audit waiver has invalid ${field}`);
    }
    if (!/^RUSTSEC-\d{4}-\d{4}$/.test(waiver.advisory)) fail(`invalid advisory ID ${waiver.advisory}`);
    if (!(waiver.expiry instanceof Date) || waiver.expiry.isDate !== true || Number.isNaN(waiver.expiry.valueOf())) {
      fail(`${waiver.advisory} expiry must be a valid TOML calendar date`);
    }
    const todayUtc = Date.UTC(today.getUTCFullYear(), today.getUTCMonth(), today.getUTCDate());
    if (waiver.expiry.valueOf() <= todayUtc) fail(`${waiver.advisory} waiver is expired`);
    if (records.has(waiver.advisory)) fail(`duplicate waiver metadata for ${waiver.advisory}`);
    records.set(waiver.advisory, waiver);
  }
  for (const advisory of ignored) {
    if (!records.has(advisory)) fail(`${advisory} is ignored without structured waiver metadata`);
  }
  for (const advisory of records.keys()) {
    if (!ignored.includes(advisory)) fail(`${advisory} waiver metadata is not present in advisories.ignore`);
  }
}

function validateReadme(text) {
  for (const required of ['SHA256SUMS', '.sigstore.json', 'sha256sum --check', 'cosign verify-blob']) {
    if (!text.includes(required)) fail(`README omitted ${required}`);
  }
  const identity = '--certificate-identity "https://github.com/adminifi-ai/allie/.github/workflows/release.yml@refs/tags/$release"';
  if (!text.includes(identity) || text.includes('--certificate-identity-regexp')) {
    fail('README must verify the exact workflow identity for the selected tag');
  }
  if (!text.includes('--certificate-oidc-issuer https://token.actions.githubusercontent.com')) {
    fail('README must constrain the GitHub Actions OIDC issuer');
  }
  if (/curl[^\n]*\|[^\n]*tar/.test(text)) fail('README must not stream an unverified download into tar');
  if (!readmeInstallBlock(text).startsWith('set -eu\n')) fail('README install flow must fail fast before download verification');
  const countAt = text.indexOf('checksum_entries=$(awk');
  const checksumAt = text.indexOf('sha256sum --check');
  const signatureAt = text.indexOf('cosign verify-blob');
  const extractionAt = text.indexOf('tar -x');
  if (!(countAt >= 0 && checksumAt > countAt && signatureAt > checksumAt && extractionAt > signatureAt)) {
    fail('README must verify checksum and signature before extraction');
  }
}

function readmeInstallBlock(text) {
  const releaseExample = text.indexOf('release=v0.1.0');
  const start = text.lastIndexOf('```sh\n', releaseExample);
  const end = text.indexOf('\n```', releaseExample);
  if (releaseExample < 0 || start < 0 || end < 0) fail('README install flow is not executable as documented');
  return text.slice(start + '```sh\n'.length, end);
}

function readmeChecksumBlock(text) {
  const install = readmeInstallBlock(text);
  const start = install.indexOf('(\n  cd "$download"\n');
  const end = install.indexOf('\n)\ncosign verify-blob', start);
  if (start < 0 || end < 0) fail('README checksum flow is not executable as documented');
  return install.slice(start, end + 2);
}

function exerciseReadmeChecksumFlow(text) {
  const block = readmeChecksumBlock(text);
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'allie-checksum-contract-'));
  const download = path.join(root, 'download');
  const bin = path.join(root, 'bin');
  const marker = path.join(root, 'sha256sum-called');
  const archive = 'allie-linux-x64.tar.gz';
  fs.mkdirSync(download);
  fs.mkdirSync(bin);
  fs.writeFileSync(path.join(download, archive), 'release archive\n');
  fs.writeFileSync(path.join(bin, 'sha256sum'), '#!/bin/sh\n: > "$SHA_MARKER"\n');
  fs.chmodSync(path.join(bin, 'sha256sum'), 0o755);

  const run = (manifest, label, expectedStatus, expectedVerifierCall) => {
    fs.writeFileSync(path.join(download, 'SHA256SUMS'), manifest);
    fs.rmSync(marker, { force: true });
    const result = spawnSync('/bin/sh', ['-eu', '-c', block], {
      env: {
        ...process.env,
        PATH: `${bin}:${process.env.PATH || ''}`,
        SHA_MARKER: marker,
        archive,
        download,
      },
      encoding: 'utf8',
    });
    const verifierCalled = fs.existsSync(marker);
    if (result.status !== expectedStatus || verifierCalled !== expectedVerifierCall) {
      fail(`${label}: status=${result.status}, verifier_called=${verifierCalled}, stderr=${result.stderr.trim()}`);
    }
  };

  try {
    const selected = `0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  ${archive}\n`;
    run(selected, 'single checksum entry', 0, true);
    run('0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  other.tar.gz\n', 'missing selected checksum entry', 1, false);
    run(selected + selected, 'duplicate selected checksum entries', 1, false);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function exerciseReadmeInstallFlow(text) {
  const block = readmeInstallBlock(text);
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'allie-install-contract-'));
  const bin = path.join(root, 'bin');
  const tarMarker = path.join(root, 'tar-called');
  const checksumMarker = path.join(root, 'checksum-called');
  const cosignMarker = path.join(root, 'cosign-called');
  fs.mkdirSync(bin);

  const stub = (name, body) => {
    const target = path.join(bin, name);
    fs.writeFileSync(target, `#!/bin/sh\n${body}\n`);
    fs.chmodSync(target, 0o755);
  };
  stub('curl', `output=$2
case "$output" in
  */SHA256SUMS) printf '%s\n' '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  allie-linux-x64.tar.gz' > "$output" ;;
  *) printf 'downloaded artifact\n' > "$output" ;;
esac`);
  stub('sha256sum', `: > "$CHECKSUM_MARKER"
exit "$CHECKSUM_STATUS"`);
  stub('cosign', `: > "$COSIGN_MARKER"
exit "$COSIGN_STATUS"`);
  stub('tar', `printf 'called\n' >> "$TAR_MARKER"`);
  stub('allie', 'exit 0');

  const run = (label, checksumStatus, cosignStatus, expectedStatus, expectedTarCalls) => {
    for (const marker of [tarMarker, checksumMarker, cosignMarker]) fs.rmSync(marker, { force: true });
    fs.rmSync(path.join(root, '.allie'), { recursive: true, force: true });
    const result = spawnSync('/bin/sh', ['-c', block], {
      cwd: root,
      env: {
        ...process.env,
        PATH: `${bin}:${process.env.PATH || ''}`,
        CHECKSUM_MARKER: checksumMarker,
        CHECKSUM_STATUS: String(checksumStatus),
        COSIGN_MARKER: cosignMarker,
        COSIGN_STATUS: String(cosignStatus),
        TAR_MARKER: tarMarker,
      },
      encoding: 'utf8',
    });
    const tarCalls = fs.existsSync(tarMarker)
      ? fs.readFileSync(tarMarker, 'utf8').trim().split('\n').filter(Boolean).length
      : 0;
    if (result.status !== expectedStatus || tarCalls !== expectedTarCalls) {
      fail(`${label}: status=${result.status}, tar_calls=${tarCalls}, stderr=${result.stderr.trim()}`);
    }
  };

  try {
    run('failed checksum blocks extraction', 1, 0, 1, 0);
    run('failed Cosign verification blocks extraction', 0, 1, 1, 0);
    run('successful verification extracts exactly once', 0, 0, 0, 1);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}


function expectRejected(action, label) {
  try {
    action();
  } catch {
    return;
  }
  fail(`negative control was accepted: ${label}`);
}

const releaseText = fs.readFileSync(RELEASE_PATH, 'utf8');
const ciText = fs.readFileSync(CI_PATH, 'utf8');
const auditText = fs.readFileSync(AUDIT_PATH, 'utf8');
const waiverText = fs.readFileSync(WAIVER_PATH, 'utf8');
const readmeText = fs.readFileSync(README_PATH, 'utf8');
validateReleaseWorkflow(releaseText);
validateCiWorkflow(ciText);
validateAuditPolicy(auditText, waiverText);
validateReadme(readmeText);
exerciseReadmeChecksumFlow(readmeText);
exerciseReadmeInstallFlow(readmeText);

const documentedAdvisory = '[advisories]\nignore = ["RUSTSEC-2099-0001"]\n';
const validWaiver = 'schema = 1\n[[waiver]]\nadvisory = "RUSTSEC-2099-0001"\ntracking_ref = "AL-999"\nrationale = "test"\nowner = "security"\nexpiry = 2099-01-01\nremoval = "upgrade"\n';
validateAuditPolicy(documentedAdvisory, validWaiver, new Date('2026-01-01T00:00:00Z'));

const releaseObject = parseYaml(releaseText, 'release workflow');
validateReleaseWorkflow(stringifyYaml(releaseObject));
const extraUpload = structuredClone(releaseObject);
extraUpload.jobs['sign-and-publish'].steps.at(-1).run = extraUpload.jobs['sign-and-publish'].steps.at(-1).run.replace(
  'dist/allie-linux-x64.tar.gz \\\n',
  'dist/allie-linux-x64.tar.gz \\\n  dist/extra.txt \\\n',
);
expectRejected(() => validateReleaseWorkflow(stringifyYaml(extraUpload)), 'extra release asset');
const handwrittenNotes = structuredClone(releaseObject);
handwrittenNotes.jobs['sign-and-publish'].steps.at(-1).run =
  handwrittenNotes.jobs['sign-and-publish'].steps.at(-1).run.replace(
    '-F generate_release_notes=true',
    "-F generate_release_notes=true \\\n  -f body='handwritten'",
  );
expectRejected(() => validateReleaseWorkflow(stringifyYaml(handwrittenNotes)), 'handwritten release notes');
const missingRepositoryContext = structuredClone(releaseObject);
missingRepositoryContext.jobs['sign-and-publish'].steps.at(-1).run =
  missingRepositoryContext.jobs['sign-and-publish'].steps.at(-1).run.replace(' --repo "$GITHUB_REPOSITORY"', '');
expectRejected(
  () => validateReleaseWorkflow(stringifyYaml(missingRepositoryContext)),
  'release command without repository context',
);
const impostorCosign = structuredClone(releaseObject);
impostorCosign.jobs['sign-and-publish'].steps.find((step) => /\/cosign-installer@/.test(String(step.uses || ''))).uses =
  'attacker/cosign-installer@6f9f17788090df1f26f669e9d70d6ae9567deba6';
expectRejected(() => validateReleaseWorkflow(stringifyYaml(impostorCosign)), 'impostor Cosign installer action');
const forgedSignature = structuredClone(releaseObject);
forgedSignature.jobs['sign-and-publish'].steps.find((step) => step.name === 'Sign release bundle with GitHub OIDC').run +=
  ' || touch dist/allie-linux-x64.tar.gz.sigstore.json';
expectRejected(() => validateReleaseWorkflow(stringifyYaml(forgedSignature)), 'forged bundle after failed signing');
const bypass = structuredClone(releaseObject);
bypass.jobs['sign-and-publish'].steps.at(-1).run += '\nfalse || true';
expectRejected(() => validateReleaseWorkflow(stringifyYaml(bypass)), '|| true bypass');
const globalPrivilege = structuredClone(releaseObject);
globalPrivilege.permissions = { contents: 'write' };
expectRejected(() => validateReleaseWorkflow(stringifyYaml(globalPrivilege)), 'global write permission');
const misplacedPrivilege = structuredClone(releaseObject);
misplacedPrivilege.jobs['build-release'].permissions['id-token'] = 'write';
expectRejected(() => validateReleaseWorkflow(stringifyYaml(misplacedPrivilege)), 'misplaced build OIDC permission');
expectRejected(
  () => validateReadme(readmeText.replace(/--certificate-identity[^\n]*/, "--certificate-identity-regexp '.*'")),
  'wildcard signing identity',
);
expectRejected(() => validateAuditPolicy('[advisories\nignore = []', waiverText), 'malformed TOML');
expectRejected(
  () => validateAuditPolicy(
    documentedAdvisory,
    'schema = 1\n[[waiver]]\nadvisory = "RUSTSEC-2099-0001"\ntracking_ref = "AL-999"\nrationale = "test"\nowner = "security"\nexpiry = 2026-02-30\nremoval = "upgrade"\n',
  ),
  'invalid calendar expiry',
);
expectRejected(
  () => validateAuditPolicy(
    documentedAdvisory,
    'schema = 1\n[[waiver]]\nadvisory = "RUSTSEC-2099-0001"\ntracking_ref = "AL-999"\nrationale = "test"\nowner = "security"\nexpiry = 2000-01-01\nremoval = "upgrade"\n',
    new Date('2026-01-01T00:00:00Z'),
  ),
  'expired waiver',
);
expectRejected(
  () => validateAuditPolicy('[advisories]\nignore = ["RUSTSEC-2099-0001"]\n', 'schema = 1\nwaiver = []\n'),
  'undocumented ignored advisory',
);

console.log('release workflow contract smoke passed with structural negative controls');
