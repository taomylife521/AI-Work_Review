import test from 'node:test';
import assert from 'node:assert/strict';
import { access, readFile } from 'node:fs/promises';

test('README 应以英文为主页并提供中文切换入口', async () => {
  const source = await readFile(new URL('./README.md', import.meta.url), 'utf8');

  assert.match(
    source,
    /href="\.\/*README\.md"[^>]*>English<\/a>/,
    'README.md（英文主页）顶部应标记 English'
  );
  assert.match(
    source,
    /href="\.\/*README\.zh\.md"[^>]*>中文<\/a>/,
    'README 顶部应提供中文切换链接（README.zh.md）'
  );

  await access(new URL('./README.zh.md', import.meta.url));
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
