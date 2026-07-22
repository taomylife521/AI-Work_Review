import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const askSource = await readFile(new URL('./Ask.svelte', import.meta.url), 'utf8');

test('Ask 流式事件按本次 assistant 消息 ID 更新，不再依赖全局最后一条 streaming', () => {
  assert.match(askSource, /createRequestEventGate/);
  assert.match(askSource, /id:\s*assistantMessageId/);
  assert.match(askSource, /updateMessageById\(assistantMessageId/);
  assert.doesNotMatch(askSource, /updateLastStreaming\(/);
});

test('Ask 请求终态或异常后会关闭事件门闩，拒绝迟到事件', () => {
  assert.match(askSource, /requestGate\.handle\(event\)/);
  assert.match(askSource, /requestGate\?\.close\(\)/);
});

test('Ask 在流式 done 后仍按消息 ID补写模型元数据', () => {
  assert.match(
    askSource,
    /updateMessageById\(assistantMessageId,[\s\S]*usedAi:[\s\S]*modelName:/,
  );
});

test('失败工具步骤使用失败文案且不会显示命中数', () => {
  assert.match(askSource, /step\.ok === false/);
  assert.match(askSource, /t\('ask\.stepFailed'\)/);
  assert.match(askSource, /step\.tool === 'search_memory' && step\.ok === true && step\.hits != null/);
});

test('Ask 将异常占位消息标记为失败，避免进入下一轮历史', () => {
  assert.match(
    askSource,
    /catch \(e\)[\s\S]*updateMessageById\(assistantMessageId,[\s\S]*failed:\s*true/,
  );
});

test('Ask 使用请求级 sending，卸载释放旧请求且旧 finally 不会释放新请求', () => {
  assert.match(askSource, /let activeSendingRequestId = null/);
  assert.match(askSource, /assistantStore\.beginSending\(assistantMessageId\)/);
  assert.match(
    askSource,
    /onDestroy\(\(\) => \{[\s\S]*assistantStore\.finishSending\(activeSendingRequestId\)/,
  );
  assert.match(
    askSource,
    /finally \{[\s\S]*assistantStore\.finishSending\(assistantMessageId\)/,
  );
  assert.doesNotMatch(askSource, /assistantStore\.setSending\(/);
});
