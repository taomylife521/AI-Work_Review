import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const read = (rel) => readFileSync(new URL(rel, import.meta.url), 'utf8');

test('密钥保险柜：敏感字段全名录登记齐全', () => {
  const secrets = read('../src-tauri/src/secrets.rs');

  assert.match(secrets, /KEYCHAIN_PLACEHOLDER: &str = "__keychain__"/);
  const requiredFields = [
    'text_model.api_key',
    'vision_model.api_key',
    'ai_provider.api_key',
    'openai_api_key',
    'assistant_search_api_key',
    'embedding_api_key',
    'telegram_bot_token',
    'feishu_app_secret',
    'feishu_verification_token',
    'feishu_encrypt_key',
    'wecom_token',
    'wecom_encoding_aes_key',
    'dingtalk_app_secret',
    'remote_storage.s3.access_key',
    'remote_storage.s3.secret_key',
    'remote_storage.webdav.password',
  ];
  for (const field of requiredFields) {
    assert.ok(secrets.includes(`"${field}"`), `secrets.rs 名录缺少 ${field}`);
  }
  // 模型档案按 id 逐个登记
  assert.match(secrets, /profile\.\{\}\.api_key|profile\.\{.*\}\.api_key|format!\("profile\.\{\}\.api_key"/);
});

test('密钥保险柜：所有落盘调用走 save_secure，载入注水已接线', () => {
  const mainSource = read('../src-tauri/src/main.rs');
  assert.match(mainSource, /mod secrets;/);
  assert.match(mainSource, /secrets::hydrate_config\(&mut config\)/);

  // src-tauri 内不得再有绕过保险柜的裸 config.save 调用
  const dirs = ['../src-tauri/src/', '../src-tauri/src/commands/'];
  for (const dir of dirs) {
    const base = fileURLToPath(new URL(dir, import.meta.url));
    for (const file of readdirSync(base)) {
      if (!file.endsWith('.rs')) continue;
      const source = readFileSync(base + file, 'utf8');
      assert.ok(
        !source.includes('.save(&config_path)'),
        `${dir}${file} 存在绕过保险柜的裸 save 调用`
      );
    }
  }
});

test('密钥保险柜：keyring 依赖已声明', () => {
  const cargo = read('../src-tauri/Cargo.toml');
  assert.match(cargo, /keyring = \{ version = "3"/);
  assert.match(cargo, /apple-native/);
  assert.match(cargo, /windows-native/);
  assert.match(cargo, /sync-secret-service/);
});
