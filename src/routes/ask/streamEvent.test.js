import { test } from 'node:test';
import assert from 'node:assert';
import { reduceStreamEvent } from './streamEvent.js';

function baseMessage(overrides = {}) {
  return {
    id: 'assistant-1',
    role: 'assistant',
    content: '',
    steps: [],
    references: [],
    toolLabels: [],
    streaming: true,
    ...overrides,
  };
}

test('stepStart 追加 running 工具步骤且不结束流', () => {
  const original = baseMessage();

  const result = reduceStreamEvent(original, {
    type: 'stepStart',
    tool: 'search_memory',
    label: '记忆检索',
  });

  assert.deepEqual(result.message.steps, [
    { tool: 'search_memory', label: '记忆检索', status: 'running' },
  ]);
  assert.equal(result.terminal, false);
  assert.deepEqual(original.steps, []);
});

test('stepResult 更新最近一个 running 且 tool 匹配的步骤并保留 ok=false', () => {
  const existingReference = { sourceId: 'old', timestamp: 1, title: '旧引用' };
  const incomingReference = { sourceId: 'new', timestamp: 2, title: '新引用' };
  const original = baseMessage({
    steps: [
      { tool: 'search_memory', status: 'done', ok: true, hits: 1 },
      { tool: 'search_memory', status: 'running' },
      { tool: 'aggregate_stats', status: 'running' },
      { tool: 'search_memory', status: 'running' },
    ],
    references: [existingReference],
  });

  const result = reduceStreamEvent(original, {
    type: 'stepResult',
    tool: 'search_memory',
    ok: false,
    hits: 0,
    references: [incomingReference],
  });

  assert.deepEqual(result.message.steps[0], original.steps[0]);
  assert.deepEqual(result.message.steps[1], original.steps[1]);
  assert.deepEqual(result.message.steps[2], original.steps[2]);
  assert.deepEqual(result.message.steps[3], {
    tool: 'search_memory',
    status: 'done',
    ok: false,
    hits: 0,
    references: [incomingReference],
  });
  assert.deepEqual(result.message.references, [existingReference, incomingReference]);
  assert.equal(result.terminal, false);
  assert.equal(original.steps[3].status, 'running');
});

test('stepResult 缺少 ok 时保留未知状态', () => {
  const original = baseMessage({
    steps: [{ tool: 'search_memory', status: 'running' }],
  });

  const result = reduceStreamEvent(original, {
    type: 'stepResult',
    tool: 'search_memory',
    hits: 2,
    references: [],
  });

  assert.equal(result.message.steps[0].ok, undefined);
  assert.equal(result.message.steps[0].hits, 2);
});

test('stepResult 缺少或非法 hits 时保留未知命中数', () => {
  for (const hits of [undefined, null, '3', Number.NaN, -1]) {
    const original = baseMessage({
      steps: [{ tool: 'search_memory', status: 'running' }],
    });

    const result = reduceStreamEvent(original, {
      type: 'stepResult',
      tool: 'search_memory',
      ok: true,
      hits,
      references: [],
    });

    assert.equal(result.message.steps[0].hits, undefined);
  }
});

test('stepResult 找不到匹配步骤时不误改其他步骤', () => {
  const original = baseMessage({
    steps: [
      { tool: 'search_memory', status: 'running' },
      { tool: 'aggregate_stats', status: 'done', ok: true, hits: 2 },
    ],
  });

  const result = reduceStreamEvent(original, {
    type: 'stepResult',
    tool: 'query_activities',
    ok: false,
    hits: 0,
    references: [],
  });

  assert.deepEqual(result.message.steps, original.steps);
  assert.equal(result.terminal, false);
});

test('token 追加文本且不结束流', () => {
  const result = reduceStreamEvent(
    baseMessage({ content: '你好' }),
    { type: 'token', token: '，世界' }
  );

  assert.equal(result.message.content, '你好，世界');
  assert.equal(result.message.streaming, true);
  assert.equal(result.terminal, false);
});

test('done 用完整答案覆盖增量文本并结束流', () => {
  const references = [{ sourceId: 'done', timestamp: 3, title: '最终引用' }];
  const result = reduceStreamEvent(
    baseMessage({
      content: '不完整答案',
      references: [{ sourceId: 'partial', timestamp: 1, title: '临时引用' }],
      toolLabels: ['旧工具'],
    }),
    {
      type: 'done',
      answer: '完整答案',
      references,
      toolLabels: ['记忆检索'],
    }
  );

  assert.equal(result.message.content, '完整答案');
  assert.deepEqual(result.message.references, references);
  assert.deepEqual(result.message.toolLabels, ['记忆检索']);
  assert.equal(result.message.streaming, false);
  assert.equal(result.terminal, true);
});

test('error 使用事件错误并结束流', () => {
  const result = reduceStreamEvent(
    baseMessage(),
    { type: 'error', error: '后端失败' },
    '请求失败'
  );

  assert.equal(result.message.content, '后端失败');
  assert.equal(result.message.streaming, false);
  assert.equal(result.message.failed, true);
  assert.equal(result.terminal, true);
});

test('error 缺少错误文本时使用 fallbackError', () => {
  const result = reduceStreamEvent(baseMessage(), { type: 'error' }, '请求失败');

  assert.equal(result.message.content, '请求失败');
  assert.equal(result.message.streaming, false);
  assert.equal(result.terminal, true);
});
