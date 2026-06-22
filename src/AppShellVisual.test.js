import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

test('应用壳层应保留左右两张主卡片，并将 stage 退回为纯布局容器', async () => {
  const [appSource, appCssSource] = await Promise.all([
    readFile(new URL('./App.svelte', import.meta.url), 'utf8'),
    readFile(new URL('./app.css', import.meta.url), 'utf8'),
  ]);

  assert.match(appSource, /app-shell/);
  assert.match(appSource, /app-shell-stage/);
  assert.match(appSource, /app-shell-sidebar-frame/);
  assert.match(appSource, /app-shell-main-frame/);
  assert.match(appSource, /app-shell-windowbar/);

  assert.match(appCssSource, /\.app-shell\b/);
  assert.match(appCssSource, /\.app-shell-stage\b/);
  assert.match(appCssSource, /\.app-shell-sidebar-frame\b/);
  assert.match(appCssSource, /\.app-shell-main-frame\b/);
  assert.match(appCssSource, /\.app-shell-windowbar\b/);
  assert.match(appCssSource, /\.app-shell-stage\s*\{[\s\S]*background:\s*transparent;/);
  assert.match(appCssSource, /\.app-shell-stage\s*\{[\s\S]*border:\s*none;/);
  assert.match(appCssSource, /\.app-shell-stage\s*\{[\s\S]*box-shadow:\s*none;/);
});

test('主导航字号应高于设置内导航，形成稳定层级', async () => {
  const appCssSource = await readFile(new URL('./app.css', import.meta.url), 'utf8');

  assert.match(appCssSource, /\.sidebar-nav-label\s*\{[\s\S]*font-size:\s*0\.98rem;/);
  assert.match(appCssSource, /\.settings-tab-rail-item\s*\{[\s\S]*font-size:\s*0\.92rem;/);
});

test('统一底板结构下不应继续保留旧的主内容外壳伪元素修补逻辑', async () => {
  const appCssSource = await readFile(new URL('./app.css', import.meta.url), 'utf8');

  assert.doesNotMatch(appCssSource, /\.app-shell-main::before/);
  assert.doesNotMatch(appCssSource, /\.dark\s+\.app-shell-main::before/);
  assert.doesNotMatch(appCssSource, /\.app-shell-windowbar::before/);
});

test('自定义窗口栏存在时，统一底板本身应整体下移，侧栏和主内容不再分别补偿顶部偏移', async () => {
  const appSource = await readFile(new URL('./App.svelte', import.meta.url), 'utf8');

  assert.match(
    appSource,
    /app-shell-stage[\s\S]*\{platform !== 'macos' \? 'pt-7' : 'pt-2'\}/
  );
  assert.doesNotMatch(
    appSource,
    /app-shell-sidebar-frame[\s\S]*\{platform !== 'macos' \? 'pt-7' : 'pt-2'\}/
  );
  assert.doesNotMatch(
    appSource,
    /app-shell-main-frame[\s\S]*\{platform !== 'macos' \? 'pt-7' : ''\}/
  );
});

test('统一底板结构下，卡片感应集中在 frame 层，内层容器不再重复叠加厚重背景与阴影', async () => {
  const appCssSource = await readFile(new URL('./app.css', import.meta.url), 'utf8');

  assert.match(appCssSource, /\.app-shell-sidebar-frame[\s\S]*background:/);
  assert.match(appCssSource, /\.app-shell-main-frame[\s\S]*background:/);
  assert.match(appCssSource, /\.app-shell-sidebar\s*\{[\s\S]*background:\s*transparent;/);
  assert.match(appCssSource, /\.app-shell-sidebar\s*\{[\s\S]*box-shadow:\s*none;/);
  assert.match(appCssSource, /\.app-shell-main\s*\{[\s\S]*background:\s*transparent;/);
  assert.match(appCssSource, /\.app-shell-main\s*\{[\s\S]*box-shadow:\s*none;/);
  assert.match(appCssSource, /\.sidebar-editorial-shell\s*\{[\s\S]*background:\s*transparent;/);
  assert.doesNotMatch(appCssSource, /\.sidebar-editorial-shell::before/);
});

test('主内容滚动条应从圆角裁剪边界内缩，避免顶部和底部被遮住', async () => {
  const appCssSource = await readFile(new URL('./app.css', import.meta.url), 'utf8');

  assert.match(appCssSource, /\.app-shell-main\s*\{[\s\S]*padding:\s*0\.18rem/);
  assert.match(appCssSource, /\.app-shell-main-scroll\s*\{[\s\S]*scrollbar-gutter:\s*stable/);
  assert.match(appCssSource, /\.app-shell-main-scroll\s*\{[\s\S]*border-radius:\s*1\.72rem/);
  assert.match(appCssSource, /\.app-shell-main-scroll\s*\{[\s\S]*padding-right:\s*0\.12rem/);
  assert.match(appCssSource, /\.app-shell-main-scroll\s*\{[\s\S]*padding-bottom:\s*0\.12rem/);
});
