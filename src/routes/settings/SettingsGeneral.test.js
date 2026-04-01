import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

test('工作时间跨零点时应显示跨天后的总时长而不是横线', async () => {
  const source = await readFile(
    new URL('./components/SettingsGeneral.svelte', import.meta.url),
    'utf8'
  );

  assert.match(source, /endTotal === startTotal/);
  assert.match(source, /endTotal < startTotal/);
  assert.match(source, /24 \* 60/);
  assert.doesNotMatch(source, /const diffSeconds = \(endTotal - startTotal\) \* 60;/);
});

test('开始时间等于结束时间时应显示零时长而不是横线', async () => {
  const source = await readFile(
    new URL('./components/SettingsGeneral.svelte', import.meta.url),
    'utf8'
  );

  assert.match(source, /endTotal === startTotal/);
  assert.match(source, /formatDurationLocalized\(0\)/);
});
