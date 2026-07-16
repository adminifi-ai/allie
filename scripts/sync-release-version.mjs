import fs from 'node:fs';
import path from 'node:path';

function fail(message) {
  throw new Error(message);
}

function replacePackageVersion(text, header, name, version, label) {
  const blocks = text.split(/(?=^\[\[?package\]?\]\]?$)/m);
  let matches = 0;
  const updated = blocks.map((block) => {
    if (!block.startsWith(header)) return block;
    if (name && !new RegExp(`^name = "${name}"$`, 'm').test(block)) return block;
    if (!/^version = "[^"]+"$/m.test(block)) fail(`${label} package block has no version`);
    matches += 1;
    return block.replace(/^version = "[^"]+"$/m, `version = "${version}"`);
  });
  if (matches !== 1) fail(`${label} must contain exactly one matching package block, found ${matches}`);
  return updated.join('');
}

function writeJson(file, update) {
  const value = JSON.parse(fs.readFileSync(file, 'utf8'));
  update(value);
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

const args = process.argv.slice(2);
const versionAt = args.indexOf('--version');
const rootAt = args.indexOf('--repo-root');
const version = versionAt >= 0 ? args[versionAt + 1] : '';
const root = path.resolve(rootAt >= 0 ? args[rootAt + 1] : '.');

if (!/^0\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)$/.test(version)) {
  fail(`release version must remain on the v0.x line, got ${version || '<missing>'}`);
}

const cargoToml = path.join(root, 'Cargo.toml');
const cargoLock = path.join(root, 'Cargo.lock');
const packageJson = path.join(root, 'package.json');
const packageLock = path.join(root, 'package-lock.json');

const manifest = fs.readFileSync(cargoToml, 'utf8');
fs.writeFileSync(
  cargoToml,
  replacePackageVersion(manifest, '[package]', '', version, 'Cargo.toml'),
);

const lock = fs.readFileSync(cargoLock, 'utf8');
fs.writeFileSync(
  cargoLock,
  replacePackageVersion(lock, '[[package]]', 'allie', version, 'Cargo.lock'),
);

writeJson(packageJson, (value) => {
  if (value.name !== 'allie-browser-worker') fail('package.json has unexpected package name');
  value.version = version;
});

writeJson(packageLock, (value) => {
  if (value.name !== 'allie-browser-worker' || value.packages?.['']?.name !== 'allie-browser-worker') {
    fail('package-lock.json has unexpected root package');
  }
  value.version = version;
  value.packages[''].version = version;
});
