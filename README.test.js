import test from 'node:test';
import assert from 'node:assert/strict';
import { access, readFile } from 'node:fs/promises';

function getPngSize(buffer) {
  assert.equal(buffer.toString('ascii', 1, 4), 'PNG');
  return {
    width: buffer.readUInt32BE(16),
    height: buffer.readUInt32BE(20),
  };
}

function getGifSize(buffer) {
  assert.match(buffer.toString('ascii', 0, 6), /^GIF8[79]a$/);
  return {
    width: buffer.readUInt16LE(6),
    height: buffer.readUInt16LE(8),
  };
}

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

test('多语言 README 底部都应展示 Star History，并在 License 后加入分隔线', async () => {
  const [zhSource, enSource, twSource] = await Promise.all([
    readFile(new URL('./README.zh.md', import.meta.url), 'utf8'),
    readFile(new URL('./README.md', import.meta.url), 'utf8'),
    readFile(new URL('./README.tw.md', import.meta.url), 'utf8'),
  ]);

  assert.match(zhSource, /## License\s+\[MIT\]\(\.\/LICENSE\)[\s\S]*---\s+## 历史星标/);
  assert.match(enSource, /## License\s+\[MIT\]\(\.\/LICENSE\)[\s\S]*---\s+## Star History/);
  assert.match(twSource, /## License\s+\[MIT\]\(\.\/LICENSE\)[\s\S]*---\s+## 歷史星標/);
  assert.match(enSource, /star-history\.com\/#wm94i\/Work-Review&Date/);
  assert.match(enSource, /<img alt="Star History" src="https:\/\/api\.star-history\.com\/svg\?repos=wm94i\/Work-Review&type=Date" width="720" \/>/);
  assert.match(zhSource, /<img alt="Star History" src="https:\/\/api\.star-history\.com\/svg\?repos=wm94i\/Work-Review&type=Date" width="720" \/>/);
  assert.match(twSource, /<img alt="Star History" src="https:\/\/api\.star-history\.com\/svg\?repos=wm94i\/Work-Review&type=Date" width="720" \/>/);
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

test('多语言 README 应覆盖当前版本关键能力和安装资产', async () => {
  const readmes = [
    {
      file: './README.md',
      patterns: [
        /Windows \| `\.exe` \/ portable `\.zip` \|/,
        /hourly activity across Today, Week, Date, and Range views/,
        /dynamic opening prompts after a model is configured/,
      ],
    },
    {
      file: './README.zh.md',
      patterns: [
        /Windows \| `\.exe` \/ 便携版 `\.zip` \|/,
        /按今日、本周、指定日期、日期范围查看小时活跃度/,
        /配置模型后显示动态开场提示/,
      ],
    },
    {
      file: './README.tw.md',
      patterns: [
        /Windows \| `\.exe` \/ 便攜版 `\.zip` \|/,
        /按今日、本週、指定日期、日期範圍查看小時活躍度/,
        /配置模型後顯示動態開場提示/,
      ],
    },
  ];

  for (const readme of readmes) {
    const source = await readFile(new URL(readme.file, import.meta.url), 'utf8');
    for (const pattern of readme.patterns) {
      assert.match(source, pattern);
    }
  }
});

test('多语言 README 应展示完整界面预览截图且图片文件存在', async () => {
  const readmes = [
    {
      file: './README.md',
      dir: 'Introduction_en',
    },
    {
      file: './README.zh.md',
      dir: 'Introduction_zh',
    },
    {
      file: './README.tw.md',
      dir: 'Introduction_tw',
    },
  ];
  const labels = [
    '概览',
    '时间线',
    '时间线详情',
    '小时总结',
    '日报',
    '助手',
    '设置-通用',
    '设置-外观',
    '设置-AI模型',
    '设置-桌面化身',
    '设置-隐私',
    '设置-存储',
    '接入管理',
    '关于',
  ];
  let expectedPngSize;
  let expectedGifSize;

  for (const readme of readmes) {
    const source = await readFile(new URL(readme.file, import.meta.url), 'utf8');
    assert.match(source, /<details>/);
    assert.match(source, /<\/details>/);
    const gifPath = `docs/${readme.dir}/工作流.gif`;
    assert.match(source, new RegExp(`<img src="${gifPath}"[^>]*width="720"`));
    const gifUrl = new URL(`./${gifPath}`, import.meta.url);
    await access(gifUrl);
    const gifSize = getGifSize(await readFile(gifUrl));
    expectedGifSize ??= gifSize;
    assert.deepEqual(gifSize, expectedGifSize);
    for (const label of labels) {
      const imagePath = `docs/${readme.dir}/${label}.png`;
      assert.match(source, new RegExp(`<img src="${imagePath}"[^>]*width="720"`));
      const imageUrl = new URL(`./${imagePath}`, import.meta.url);
      await access(imageUrl);
      const pngSize = getPngSize(await readFile(imageUrl));
      expectedPngSize ??= pngSize;
      assert.deepEqual(pngSize, expectedPngSize);
    }
  }
});

test('多语言 README 的社区图片应使用统一规格展示资产', async () => {
  const sources = await Promise.all([
    readFile(new URL('./README.md', import.meta.url), 'utf8'),
    readFile(new URL('./README.zh.md', import.meta.url), 'utf8'),
    readFile(new URL('./README.tw.md', import.meta.url), 'utf8'),
  ]);
  const imagePaths = [
    'docs/group/wechat-group.png',
    'docs/group/official-account.png',
  ];
  let expectedSize;

  for (const source of sources) {
    assert.doesNotMatch(source, /docs\/group\/vx\.jpg/);
    assert.doesNotMatch(source, /docs\/group\/gzh\.jpg/);
    for (const imagePath of imagePaths) {
      assert.match(source, new RegExp(`<img src="${imagePath}"[^>]*width="220"`));
    }
  }

  for (const imagePath of imagePaths) {
    const imageUrl = new URL(`./${imagePath}`, import.meta.url);
    await access(imageUrl);
    const size = getPngSize(await readFile(imageUrl));
    expectedSize ??= size;
    assert.deepEqual(size, expectedSize);
  }
});
