import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

test('助手页应渲染工作研究台结构', async () => {
  const [source, appCssSource] = await Promise.all([
    readFile(new URL('./Ask.svelte', import.meta.url), 'utf8'),
    readFile(new URL('../../app.css', import.meta.url), 'utf8'),
  ]);

  assert.match(source, /ask-workbench-shell/);
  assert.match(source, /ask-welcome-panel/);
  assert.match(source, /ask-composer-shell/);
  assert.match(source, /ask-reference-card/);

  const welcomePanel = source.match(/<div class="ask-welcome-panel[^"]*"/)?.[0] ?? '';
  assert.ok(welcomePanel);
  assert.doesNotMatch(welcomePanel, /min-h-full|justify-center/);
  assert.match(appCssSource, /\.ask-welcome-panel\s*\{[^}]*min-height:\s*32rem[^}]*padding:\s*clamp\(/);
  assert.match(appCssSource, /\.ask-starter-grid\s*\{[^}]*position:\s*relative/);
  assert.doesNotMatch(appCssSource, /\.ask-starter-grid\s*\{[^}]*(?:position:\s*absolute|margin(?:-top)?:\s*-)/);
});
