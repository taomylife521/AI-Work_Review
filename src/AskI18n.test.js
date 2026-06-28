import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const CJK = /[一-鿿]/;

// #116 专项：Ask 助手页英文模式中文残留排查（静态扫描，防回归）。
// 运行时 AI 动态生成的 starter/回答不在此覆盖范围（由模型行为决定）。

test('Ask.svelte 无硬编码中文 UI 文本（英文模式不泄漏）', async () => {
  const src = await readFile(
    new URL('../src/routes/ask/Ask.svelte', import.meta.url),
    'utf8',
  );
  const offenders = [];
  src.split('\n').forEach((raw, i) => {
    const line = raw.trim();
    if (!CJK.test(line)) return;
    // 合法中文：注释 / 日志 / provider 三语结构值 / 段落匹配关键词 / t()/tm() 调用
    if (/^(\/\/|\*|\/\*|<!--)/.test(line)) return;
    if (/console\.|devLog\(/.test(line)) return;
    if (/'zh-CN'\s*:|'zh-TW'\s*:/.test(line)) return;
    if (/[ (]t\(['"]|[ (]tm\(['"]|translateCategory/.test(line)) return;
    if (
      /结论|依据|关键发现|本期概览|重点工作|核心观察|风险与提醒|下阶段建议|工作复盘|主要意图|主要工作|待跟进事项|代表性 Session|相关记录依据|我基于周报复盘|我基于意图识别|我基于 Session 聚合|我基于记忆检索/.test(
        line,
      )
    ) {
      return;
    }
    offenders.push(`${i + 1}: ${line}`);
  });
  assert.deepEqual(
    offenders,
    [],
    `Ask.svelte 疑似硬编码中文（英文模式会泄漏）:\n${offenders.join('\n')}`,
  );
});

test('Ask 每个 provider 都含 en 字段（英文模式不 fallback 到中文）', async () => {
  const src = await readFile(
    new URL('../src/routes/ask/Ask.svelte', import.meta.url),
    'utf8',
  );
  const start = src.indexOf('providerDisplayNames');
  const block = start >= 0 ? src.slice(start, src.indexOf('};', start)) : '';
  const providerCount = (block.match(/^\s{4}\w+:\s*\{$/gm) || []).length;
  const enCount = (block.match(/^\s+en:\s*'/gm) || []).length;
  assert.ok(
    providerCount > 0 && enCount === providerCount,
    `provider 数(${providerCount}) != en 字段数(${enCount})，英文模式会 fallback 到中文`,
  );
});
