import test from 'node:test';
import assert from 'node:assert/strict';

test('高分辨率图标持久化应受总字符预算限制', async () => {
  const hadWindow = Object.hasOwn(globalThis, 'window');
  const previousWindow = globalThis.window;
  let latestPersistedValue = '';
  let persistedCount = 0;
  let resolveAllPersisted;
  const allPersisted = new Promise((resolve) => {
    resolveAllPersisted = resolve;
  });
  globalThis.window = {
    localStorage: {
      getItem: () => null,
      setItem: (_key, value) => {
        latestPersistedValue = value;
        persistedCount += 1;
        if (persistedCount === 3) resolveAllPersisted();
      },
    },
  };

  try {
    const moduleUrl = new URL(`./iconCache.js?budget-test=${Date.now()}`, import.meta.url);
    const { loadAppIcon } = await import(moduleUrl.href);
    const largeIcon = 'a'.repeat(700_000);
    const invoke = async () => largeIcon;

    loadAppIcon('First App', invoke);
    loadAppIcon('Second App', invoke);
    loadAppIcon('Third App', invoke);
    let timeoutId;
    await Promise.race([
      allPersisted,
      new Promise((_, reject) => {
        timeoutId = setTimeout(() => reject(new Error('等待图标持久化超时')), 1_000);
      }),
    ]);
    clearTimeout(timeoutId);

    assert.ok(latestPersistedValue.length > 0, '应写入至少一个有效图标');
    assert.ok(
      latestPersistedValue.length <= 1_500_000,
      `持久化内容不得超过字符预算，实际为 ${latestPersistedValue.length}`,
    );
    const persisted = JSON.parse(latestPersistedValue);
    assert.deepEqual(
      persisted.items.map((item) => item.key),
      ['Second App', 'Third App'],
      '应以旧到新的顺序保留预算内最近使用的图标',
    );
  } finally {
    if (hadWindow) {
      globalThis.window = previousWindow;
    } else {
      delete globalThis.window;
    }
  }
});
