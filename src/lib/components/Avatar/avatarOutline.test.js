import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { formatBubbleMessage } from './bubbleMessage.js';

test('桌宠应直接复用开源 BongoCat 图层，而不是继续手绘 SVG 轮廓', () => {
  const source = readFileSync(new URL('./AvatarCanvas.svelte', import.meta.url), 'utf8');
  const registrySource = readFileSync(new URL('./avatarPresetRegistry.js', import.meta.url), 'utf8');

  assert.doesNotMatch(source, /getAvatarOutline/);
  assert.doesNotMatch(source, /AVATAR_OUTLINE_LAYOUT/);
  assert.match(registrySource, /assets\/bongocat\/mouse-bg\.png/);
  assert.match(registrySource, /assets\/bongocat\/standard-keyboard-0\.png/);
  assert.match(registrySource, /assets\/bongocat\/standard-hand-0\.png/);
  assert.match(registrySource, /assets\/bongocat\/standard-up\.png/);
  assert.match(source, /<svg/);
  assert.match(source, /<image/);
  assert.match(source, /sceneSrc/);
  assert.match(source, /standardHandSrc/);
  assert.match(source, /keyOverlaySrc/);
  assert.match(source, /computeStandardMouseGeometry/);
  assert.match(source, /mouseArmPoints/);
  assert.match(source, /getAvatarPresetDefinition/);
  assert.match(source, /showKeyboardLayers/);
  assert.match(source, /frameIndex/);
});

test('BongoCat 资源文件和来源说明应落在桌宠组件目录内', () => {
  assert.equal(
    existsSync(new URL('./assets/bongocat/standard-bg.png', import.meta.url)),
    true
  );
  assert.equal(
    existsSync(new URL('./assets/bongocat/mouse-bg.png', import.meta.url)),
    true
  );
  assert.equal(
    existsSync(new URL('./assets/bongocat/standard-hand-0.png', import.meta.url)),
    true
  );
  assert.equal(
    existsSync(new URL('./assets/bongocat/model-standard/background.png', import.meta.url)),
    true
  );
  assert.equal(
    existsSync(new URL('./assets/bongocat/model-standard/cover.png', import.meta.url)),
    true
  );
  assert.equal(
    existsSync(new URL('./assets/bongocat/model-gamepad/background.png', import.meta.url)),
    true
  );
  assert.equal(
    existsSync(new URL('./assets/bongocat/model-gamepad/cover.png', import.meta.url)),
    true
  );
  assert.equal(
    existsSync(new URL('./assets/bongocat/standard-keyboard-0.png', import.meta.url)),
    true
  );
  assert.equal(
    existsSync(new URL('./assets/bongocat/standard-up.png', import.meta.url)),
    true
  );
  assert.equal(
    existsSync(new URL('./assets/bongocat/README.md', import.meta.url)),
    true
  );
});

test('桌宠路径不应复用全局 outline 类名，避免 SVG 包围框泄漏', () => {
  const source = readFileSync(new URL('./AvatarCanvas.svelte', import.meta.url), 'utf8');
  const windowSource = readFileSync(new URL('../../../routes/avatar/AvatarWindow.svelte', import.meta.url), 'utf8');
  const engineSource = readFileSync(new URL('../../../../src-tauri/src/avatar_engine.rs', import.meta.url), 'utf8');

  assert.doesNotMatch(source, /class="outline"/);
  assert.match(source, /avatar-shell/);
  assert.match(source, /--avatar-shell-opacity/);
  assert.match(source, /getAvatarIdleMotionMeta/);
  assert.match(source, /state\.contextLabel/);
  assert.match(source, /motionBeat/);
  assert.match(source, /transitionClass/);
  assert.match(source, /idle-breathe/);
  assert.match(source, /transition-focus-shift/);
  assert.match(source, /scene-svg/);
  assert.match(source, /pointer-events:\s*none/);
  assert.match(source, /showKeyboardLayers/);
  assert.match(source, /Math\.floor\(motionBeat \/ 2\)/);
  assert.match(source, /keyboard-layer/);
  assert.match(source, /standard-hand-layer/);
  assert.match(source, /mouse-arm-fill/);
  assert.match(source, /mouse-device-layer/);
  assert.doesNotMatch(windowSource, /aria-label="打开主界面"/);
  assert.match(windowSource, /on:avatarpointerdown=\{startAvatarDrag\}/);
  assert.match(windowSource, /on:avataractivate=\{openMainWindow\}/);
  assert.match(windowSource, /getAvatarTransitionMeta/);
  assert.match(windowSource, /getAvatarMotionStepDelay/);
  assert.match(windowSource, /motionBeat = \(motionBeat \+ 1\) % 96/);
  assert.match(windowSource, /scheduleNextMotionStep/);
  assert.match(windowSource, /transitionClass = transition\.className/);
  assert.match(windowSource, /getAvatarStateBubble/);
  assert.match(windowSource, /showBubble\(stateBubble\)/);
  assert.match(
    windowSource,
    /nextState\.mode !== state\.mode\s*\|\|\s*nextState\.contextLabel !== state\.contextLabel/
  );
  assert.match(engineSource, /const AVATAR_SCALE_DEFAULT: f64 = 0\.9;/);
  assert.match(engineSource, /const AVATAR_WINDOW_BASE_WIDTH: f64 = 276\.0;/);
  assert.match(engineSource, /const AVATAR_WINDOW_BASE_HEIGHT: f64 = 248\.0;/);
  assert.match(engineSource, /const AVATAR_WINDOW_MARGIN: f64 = 8\.0;/);
});

test('状态气泡应悬浮在猫头上方，采用紧凑气泡而不是横条', () => {
  const source = readFileSync(new URL('./AvatarPopover.svelte', import.meta.url), 'utf8');
  const windowSource = readFileSync(new URL('../../../routes/avatar/AvatarWindow.svelte', import.meta.url), 'utf8');

  assert.doesNotMatch(source, /Array\.from\(bubble\.message\.replace\(\/\\s\+\/g, ''\)\)/);
  assert.match(source, /style="right: 10%; top: 6%;"/);
  assert.doesNotMatch(source, /writing-mode: vertical-rl/);
  assert.match(source, /width: fit-content/);
  assert.match(source, /max-width: min\(62vw, 228px\)/);
  assert.match(source, /min-width: 118px/);
  assert.match(source, /display: inline-flex;/);
  assert.match(source, /background:\s*rgba\(255,\s*255,\s*255,\s*0\.96\)/);
  assert.match(source, /color:\s*rgb\(15,\s*23,\s*42\)/);
  assert.match(source, /linear-gradient\(180deg,\s*rgba\(236,\s*253,\s*245,\s*0\.98\),\s*rgba\(209,\s*250,\s*229,\s*0\.95\)\)/);
  assert.match(source, /shadow-\[0_8px_22px_rgba\(15,23,42,0\.12\),0_2px_8px_rgba\(15,23,42,0\.08\)\]/);
  assert.match(source, /innerPanelStyle/);
  assert.match(source, /text-\[13px\] font-medium leading-\[1\.45\] tracking-\[0\.01em\]/);
  assert.match(source, /rounded-\[20px\]/);
  assert.match(source, /overflow-wrap: anywhere/);
  assert.match(source, /white-space: pre-wrap/);
  assert.match(source, /bubble-tail/);
  assert.match(windowSource, /h-\[86px\]/);
  assert.match(windowSource, /top-\[78px\]/);
  assert.match(windowSource, /class="h-full w-\[82%\]"/);
  assert.doesNotMatch(source, /-translate-x-1\/2/);
});

test('休息提醒气泡应支持常驻显示和手动关闭', () => {
  const source = readFileSync(new URL('./AvatarPopover.svelte', import.meta.url), 'utf8');
  const windowSource = readFileSync(new URL('../../../routes/avatar/AvatarWindow.svelte', import.meta.url), 'utf8');

  assert.match(source, /export let onClose = \(\) => \{\};/);
  assert.match(source, /bubble\?\.persistent/);
  assert.match(source, /class="absolute inset-0 z-20 overflow-visible pointer-events-none"/);
  assert.match(source, /class="pointer-events-auto relative rounded-\[20px\]/);
  assert.match(source, /<button[\s\S]*type="button"[\s\S]*on:click=\{onClose\}/);
  assert.match(source, /aria-label="关闭提醒"/);
  assert.match(windowSource, /<AvatarPopover \{bubble\} onClose=\{dismissBubble\} \/>/);
  assert.match(windowSource, /if \(!payload\?\.persistent\)/);
});

test('英文桌宠气泡文案应保留单词间空格', () => {
  assert.equal(
    formatBubbleMessage('Time for a break. Stand up and stretch a bit.'),
    'Time for a break. Stand up and stretch a bit.'
  );
});
