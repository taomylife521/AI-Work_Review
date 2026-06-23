import test from 'node:test';
import assert from 'node:assert/strict';
import { access, readFile } from 'node:fs/promises';

test('README 语言切换应突出当前语言且只链接其他语言', async () => {
  const [enSource, zhSource, twSource] = await Promise.all([
    readFile(new URL('./README.md', import.meta.url), 'utf8'),
    readFile(new URL('./README.zh.md', import.meta.url), 'utf8'),
    readFile(new URL('./README.tw.md', import.meta.url), 'utf8'),
  ]);

  assert.match(
    enSource,
    /<strong>English<\/strong> · <a href="\.\/*README\.zh\.md">简体中文<\/a> · <a href="\.\/*README\.tw\.md">繁體中文<\/a>/,
    '英文 README 应高亮 English，并只链接简体/繁体 README'
  );
  assert.doesNotMatch(enSource, /href="\.\/*README\.md"[^>]*>English<\/a>/);

  assert.match(
    zhSource,
    /<a href="\.\/*README\.md">English<\/a> · <strong>简体中文<\/strong> · <a href="\.\/*README\.tw\.md">繁體中文<\/a>/,
    '简体 README 应高亮简体中文，并只链接英文/繁体 README'
  );
  assert.doesNotMatch(zhSource, /href="\.\/*README\.zh\.md"[^>]*>简体中文<\/a>/);

  assert.match(
    twSource,
    /<a href="\.\/*README\.md">English<\/a> · <a href="\.\/*README\.zh\.md">简体中文<\/a> · <strong>繁體中文<\/strong>/,
    '繁体 README 应高亮繁體中文，并只链接英文/简体 README'
  );
  assert.doesNotMatch(twSource, /href="\.\/*README\.tw\.md"[^>]*>繁體中文<\/a>/);

  await access(new URL('./README.zh.md', import.meta.url));
  await access(new URL('./README.tw.md', import.meta.url));
});

test('中英文 README 底部都应展示 Star History，并在 License 后加入分隔线', async () => {
  const [zhSource, enSource] = await Promise.all([
    readFile(new URL('./README.zh.md', import.meta.url), 'utf8'),
    readFile(new URL('./README.md', import.meta.url), 'utf8'),
  ]);

  assert.match(zhSource, /## License\s+\[MIT\]\(\.\/LICENSE\)[\s\S]*---\s+## 历史星标/);
  assert.match(enSource, /## License\s+\[MIT\]\(\.\/LICENSE\)[\s\S]*---\s+## Star History/);
  assert.match(enSource, /star-history\.com\/#wm94i\/Work-Review&Date/);
});

test('README 不应把默认关闭的 Localhost API 描述为启动后自动开放', async () => {
  const sources = await Promise.all([
    readFile(new URL('./README.md', import.meta.url), 'utf8'),
    readFile(new URL('./README.zh.md', import.meta.url), 'utf8'),
    readFile(new URL('./README.tw.md', import.meta.url), 'utf8'),
  ]);

  for (const source of sources) {
    assert.doesNotMatch(source, /automatically exposes a local HTTP API after launch/);
    assert.doesNotMatch(source, /应用启动后自动在本地开放 HTTP API/);
    assert.doesNotMatch(source, /應用啟動後自動在本地開放 HTTP API/);
  }
});
