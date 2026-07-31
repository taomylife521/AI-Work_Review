import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

test('Release workflow 应在构建前执行测试并使用 npm ci', () => {
  const source = readFileSync(new URL('./.github/workflows/release.yml', import.meta.url), 'utf8');

  assert.match(source, /run:\s*npm ci/);
  assert.match(source, /name:\s*Run frontend tests/);
  assert.match(source, /run:\s*node --test/);
  assert.match(source, /name:\s*Build frontend assets for Rust tests/);
  assert.match(source, /run:\s*npm run build/);
  assert.match(source, /name:\s*Run Rust tests/);
  assert.match(source, /run:\s*cargo test --manifest-path src-tauri\/Cargo\.toml/);

  const frontendIndex = source.indexOf('name: Run frontend tests');
  const frontendBuildIndex = source.indexOf('name: Build frontend assets for Rust tests');
  const rustIndex = source.indexOf('name: Run Rust tests');
  const buildIndex = source.indexOf('name: Build application');

  assert.notEqual(frontendIndex, -1);
  assert.notEqual(frontendBuildIndex, -1);
  assert.notEqual(rustIndex, -1);
  assert.notEqual(buildIndex, -1);
  assert.ok(frontendIndex < buildIndex, '前端测试必须先于构建执行');
  assert.ok(frontendBuildIndex < rustIndex, 'Rust 测试前必须先生成 frontendDist');
  assert.ok(rustIndex < buildIndex, 'Rust 测试必须先于构建执行');
});

test('macOS release 必须使用稳定签名并拒绝 ad-hoc 产物', () => {
  const source = readFileSync(new URL('./.github/workflows/release.yml', import.meta.url), 'utf8');
  const setupSource = readFileSync(new URL('./scripts/setup-codesign.sh', import.meta.url), 'utf8');

  assert.match(source, /name:\s*Import stable macOS code signing certificate/);
  assert.match(source, /brew --prefix openssl@3/);
  assert.match(source, /pkcs12[\s\\]*-export[\s\\]*-legacy/);
  assert.match(source, /security add-trusted-cert -r trustRoot -p codeSign/);
  assert.match(source, /security find-identity -v -p codesigning/);
  assert.match(source, /APPLE_SIGNING_IDENTITY=\$identity_hash/);
  assert.match(source, /MACOS_CODESIGN_AUTHORITY=\$CERT_NAME/);
  assert.match(source, /name:\s*Verify stable macOS code signature/);
  assert.match(source, /Signature=adhoc/);
  assert.doesNotMatch(source, /export APPLE_SIGNING_IDENTITY="-"/);

  assert.match(setupSource, /security add-trusted-cert -r trustRoot -p codeSign/);
  assert.match(setupSource, /security find-identity -v -p codesigning/);
  assert.match(setupSource, /PKCS12_ARGS=\(-legacy\)/);
});

test('Release workflow 应构建并上传 Linux RPM 产物', () => {
  const source = readFileSync(new URL('./.github/workflows/release.yml', import.meta.url), 'utf8');

  assert.match(source, /args:\s*"--target x86_64-unknown-linux-gnu --bundles deb,rpm,appimage"[\s\S]*target:\s*x86_64-unknown-linux-gnu/);
  assert.match(source, /args:\s*"--target aarch64-unknown-linux-gnu --bundles deb"[\s\S]*target:\s*aarch64-unknown-linux-gnu/);
  assert.match(source, /sudo apt-get install -y[\s\S]*\brpm\b/);
  assert.match(source, /-name "\*\.rpm"/);
  assert.match(source, /release\/bundle\/rpm\/\*\.rpm/);
  assert.match(source, /require_file "\*\/release\/bundle\/rpm\/\*\.rpm" "Linux x64 RPM"/);
  assert.match(source, /target\/\*\*\/release\/bundle\/rpm\/\*\.rpm/);
});
