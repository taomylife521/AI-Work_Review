import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

test('时间线详情应支持修改应用默认分类并二次确认后回填历史', async () => {
  const source = await readFile(new URL('./Timeline.svelte', import.meta.url), 'utf8');

  assert.match(source, /invoke\('set_app_category_rule'/);
  assert.match(source, /timeline\.changeCategoryMessage/);
  assert.match(source, /timeline\.detail\.appCategoryHelp/);
  assert.match(source, /pendingChangeCategory/);
  assert.match(source, /doChangeAppCategory/);
});

test('时间线详情分类选择器应按当前语言翻译内置分类', async () => {
  const source = await readFile(new URL('./Timeline.svelte', import.meta.url), 'utf8');

  assert.match(source, /translatedCategoryName = translateCategoryLabel\(cat\.key\)/);
  assert.match(source, /isKnownSystemCategory = cat\.is_system \|\| translatedCategoryName !== cat\.key/);
  assert.match(
    source,
    /return isKnownSystemCategory \? translatedCategoryName : \(cat\.name \|\| translatedCategoryName\)/
  );
  assert.doesNotMatch(
    source,
    /function getCategoryDisplayName\(cat\) \{[\s\S]*return cat\.name \|\| translateCategoryLabel\(cat\.key\);[\s\S]*\}/,
    '分类选择器不能直接优先显示 get_categories 返回的中文内置分类名'
  );
});
