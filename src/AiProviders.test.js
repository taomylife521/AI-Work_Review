import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';

async function readCommandsSource() {
  // commands.rs 已按领域拆分为 commands/*.rs，这里拼接所有子模块以保持断言语义不变。
  const dir = new URL('../src-tauri/src/commands/', import.meta.url);
  const files = (await readdir(dir)).filter((f) => f.endsWith('.rs'));
  const parts = await Promise.all(files.map((f) => readFile(new URL(f, dir), 'utf8')));
  return parts.join('\n');
}

test('应提供 MiniMax 作为新的 AI 提供商并同步到文档', async () => {
  const [configSource, commandSource, readmeSource, readmeEnSource] = await Promise.all([
    readFile(new URL('../crates/core/src/config.rs', import.meta.url), 'utf8'),
    readCommandsSource(),
    readFile(new URL('../README.zh.md', import.meta.url), 'utf8'),
    readFile(new URL('../README.md', import.meta.url), 'utf8'),
  ]);

  assert.match(configSource, /MiniMax/);
  assert.match(configSource, /https:\/\/api\.minimaxi\.com\/v1/);
  assert.match(commandSource, /稀宇科技 MiniMax/);
  assert.match(commandSource, /MiniMax-M2\.5/);
  assert.match(readmeSource, /MiniMax/);
  assert.match(readmeEnSource, /MiniMax/);
});
