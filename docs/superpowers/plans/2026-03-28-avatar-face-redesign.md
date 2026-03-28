# Avatar Face Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 重做桌宠脸部视觉语言，把当前符号化猫脸升级为以 `A · 平衡版` 为基线、并在其基础上让眼睛再大一点的二次元萌系猫咪脸，同时保证不同状态下仍然稳定好看。

**Architecture:** 保持现有桌宠窗口、动作状态机和交互逻辑不变，只重构前端 SVG 绘制层。先用测试锁定新的脸部结构契约，再改 `avatarOutline.js` 与 `AvatarCanvas.svelte` 的静态脸部层，最后把 `avatarStateMeta.js` 的状态差异迁移到新的大眼猫脸基线之上。执行时必须以已确认的 `A · 平衡版` 为视觉锚点，只把眼睛进一步放大，不能把脸整体改得更圆更幼。

**Tech Stack:** Svelte、SVG Path、Node.js `node:test`

---

## 文件结构

- `src/lib/components/Avatar/avatarOutline.js`
  负责头部、耳朵和新增鼻口区等静态轮廓路径，是新脸型的几何基线。
- `src/lib/components/Avatar/AvatarCanvas.svelte`
  负责把轮廓路径和状态元数据组合成最终桌宠 SVG，承载眼睛、鼻口区、嘴型和样式类。
- `src/lib/components/Avatar/avatarStateMeta.js`
  负责不同模式与上下文下的脸部细节差异，包括新眼型、高光、嘴型和轻量神态变化。
- `src/lib/components/Avatar/avatarOutline.test.js`
  负责锁定轮廓层和画布层的新脸部结构契约，避免改脸时回退到旧的符号化实现。
- `src/lib/components/Avatar/avatarStateMeta.test.js`
  负责锁定不同模式的脸部元数据差异，确保“统一底脸 + 小幅表情变化”成立。

## 视觉锚点

- 已确认视觉方向：`A · 平衡版`
- 已确认微调方向：`眼睛再大一点`
- 不允许偏移的方向：
  - 不把脸整体继续做圆，避免偏离 A 版的平衡感
  - 不把气质改成低幼表情包猫
  - 不为了放大眼睛而削弱鼻口区

### Task 1: 用测试锁定新脸部契约

**Files:**
- Modify: `src/lib/components/Avatar/avatarOutline.test.js`
- Modify: `src/lib/components/Avatar/avatarStateMeta.test.js`

- [ ] **Step 1: 先在轮廓测试里写出新脸部结构的失败断言**

```js
test('桌宠轮廓应暴露新的脸部结构路径，并在画布中使用新的脸部图层', () => {
  const outline = getAvatarOutline();
  const source = readFileSync(new URL('./AvatarCanvas.svelte', import.meta.url), 'utf8');

  assert.equal(typeof outline.muzzlePath, 'string');
  assert.equal(typeof outline.nosePath, 'string');
  assert.match(outline.muzzlePath, /Q110/);
  assert.match(outline.nosePath, /L110 11[0-6]/);

  assert.match(source, /class="muzzle-fill"/);
  assert.match(source, /class="eye-fill"/);
  assert.match(source, /class="eye-highlight"/);
  assert.match(source, /class="nose-fill"/);
  assert.doesNotMatch(source, /M67 110 H82 M118 110 H133/);
});
```

- [ ] **Step 2: 运行轮廓测试，确认它先失败**

Run: `node --test src/lib/components/Avatar/avatarOutline.test.js`

Expected: FAIL，报错应包含以下任一信息：
- `outline.muzzlePath` 为 `undefined`
- `class="muzzle-fill"` 未匹配
- 旧胡须直线仍然存在

- [ ] **Step 3: 在状态元数据测试里写出新脸部元信息的失败断言**

```js
test('桌宠状态元信息应建立在统一底脸上，并提供新的眼部高光参数', () => {
  const idle = getAvatarModeMeta('idle');
  const working = getAvatarModeMeta('working', '编码中');
  const reading = getAvatarModeMeta('reading', '调研中');

  assert.equal(typeof idle.eyeHighlightPath, 'string');
  assert.match(idle.eyePath, /Z/);
  assert.match(idle.eyeHighlightPath, /Q/);
  assert.notEqual(working.eyePath, idle.eyePath);
  assert.notEqual(reading.eyeHighlightPath, idle.eyeHighlightPath);
  assert.equal(idle.tailClass, 'tail-idle');
});
```

- [ ] **Step 4: 运行状态元数据测试，确认它先失败**

Run: `node --test src/lib/components/Avatar/avatarStateMeta.test.js`

Expected: FAIL，报错应包含以下任一信息：
- `eyeHighlightPath` 为 `undefined`
- `idle.eyePath` 不包含封闭路径
- 新旧脸部参数不匹配

- [ ] **Step 5: 提交测试契约**

```bash
git add src/lib/components/Avatar/avatarOutline.test.js src/lib/components/Avatar/avatarStateMeta.test.js
git commit -m "test: lock avatar face redesign contract"
```

### Task 2: 重画静态脸型并改造 SVG 图层

**Files:**
- Modify: `src/lib/components/Avatar/avatarOutline.js`
- Modify: `src/lib/components/Avatar/AvatarCanvas.svelte`
- Test: `src/lib/components/Avatar/avatarOutline.test.js`

- [ ] **Step 1: 在 `avatarOutline.js` 里添加新脸型必需的静态路径**

```js
export function getAvatarOutline() {
  return {
    headPath:
      'M58 84 L70 48 C75 31 90 24 106 27 C113 28 119 31 124 36 L131 30 C143 23 156 29 161 43 L171 80 C175 90 177 100 177 111 C177 142 149 162 110 162 C71 162 43 143 43 111 C43 100 46 90 58 84 Z',
    bodyPath:
      'M72 111 C70 127 73 147 81 159 L121 160 C128 149 130 129 128 111 C123 98 112 92 100 92 C88 92 77 98 72 111 Z',
    tailPath:
      'M142 124 C154 106 171 108 171 127 C171 144 159 154 146 150 C153 145 158 137 158 128 C158 119 153 113 145 118 Z',
    leftPawPath:
      'M67 145 C59 154 59 166 67 172 C75 176 82 169 82 160 C82 151 77 145 67 145 Z',
    rightPawPath:
      'M133 145 C141 154 141 166 133 172 C125 176 118 169 118 160 C118 151 123 145 133 145 Z',
    leftEarInnerPath: 'M73 77 L82 50 L96 76',
    rightEarInnerPath: 'M126 76 L139 49 L148 78',
    muzzlePath:
      'M84 116 Q91 107 110 107 Q129 107 136 116 Q132 133 110 139 Q88 133 84 116 Z',
    nosePath: 'M104 108 L110 114 L116 108 Q110 104 104 108 Z',
  };
}
```

- [ ] **Step 2: 在 `AvatarCanvas.svelte` 里把旧符号脸替换成新图层**

```svelte
$: eyeHighlightPath = resolvedMeta.eyeHighlightPath;

<g class={headClass}>
  <path d={outline.headPath} class="avatar-hit avatar-fill avatar-stroke" />
  <path d={outline.leftEarInnerPath} class="ear-detail" />
  <path d={outline.rightEarInnerPath} class="ear-detail" />
  <path d={outline.muzzlePath} class="muzzle-fill" />
  <ellipse cx="84" cy="121" rx="10.5" ry="6.5" class="cheek-detail" />
  <ellipse cx="136" cy="121" rx="10.5" ry="6.5" class="cheek-detail" />
  <path d={eyePath} class="eye-fill" />
  <path d={eyeHighlightPath} class="eye-highlight" />
  <path d={outline.nosePath} class="nose-fill" />
  <path d={mouthPath} class="face-line mouth-line" />
</g>
```

- [ ] **Step 3: 给新脸部图层补样式，并删除旧的符号化直线胡须**

```css
.muzzle-fill,
.eye-fill,
.eye-highlight,
.nose-fill {
  pointer-events: none;
}

.muzzle-fill {
  fill: rgba(255, 250, 252, 0.96);
  stroke: rgba(30, 41, 59, 0.16);
  stroke-width: 1.8;
}

.eye-fill {
  fill: rgba(30, 41, 59, 0.96);
  stroke: none;
}

.eye-highlight {
  fill: none;
  stroke: rgba(255, 255, 255, 0.92);
  stroke-width: 2.6;
  stroke-linecap: round;
}

.nose-fill {
  fill: rgba(225, 147, 163, 0.96);
  stroke: none;
}

.mouth-line {
  stroke-width: 3.4;
}
```

- [ ] **Step 4: 运行轮廓测试，确认新结构全部通过**

Run: `node --test src/lib/components/Avatar/avatarOutline.test.js`

Expected: PASS，输出应包含 `ok`，且不再出现 `muzzlePath` 或 `muzzle-fill` 缺失。

- [ ] **Step 5: 提交静态脸型与画布改造**

```bash
git add src/lib/components/Avatar/avatarOutline.js src/lib/components/Avatar/AvatarCanvas.svelte src/lib/components/Avatar/avatarOutline.test.js
git commit -m "feat: redraw avatar face structure"
```

### Task 3: 把状态元数据迁移到新大眼猫脸基线

**Files:**
- Modify: `src/lib/components/Avatar/avatarStateMeta.js`
- Modify: `src/lib/components/Avatar/avatarStateMeta.test.js`
- Test: `src/lib/components/Avatar/avatarOutline.test.js`
- Test: `src/lib/components/Avatar/avatarStateMeta.test.js`

- [ ] **Step 1: 先把默认状态改成新的大眼猫脸元数据**

```js
const MODE_META = {
  idle: {
    eyePath:
      'M67 95 Q78 77 95 91 Q89 104 67 95 Z M125 91 Q142 77 153 95 Q131 104 125 91 Z',
    eyeHighlightPath:
      'M80 88 Q86 84 92 88 M128 88 Q134 84 140 88',
    mouthPath: 'M100 121 Q110 128 120 121',
    leftPawClass: 'paw-rest',
    rightPawClass: 'paw-rest',
    shellClass: 'mode-idle',
    tailClass: 'tail-idle',
    earTone: 'rgba(248, 218, 214, 0.92)',
    cheekTone: 'rgba(251, 214, 218, 0.52)',
    cheekOpacity: 0.34,
  },
  working: {
    eyePath:
      'M68 96 Q79 80 95 92 Q89 103 68 96 Z M125 92 Q141 80 152 96 Q131 103 125 92 Z',
    eyeHighlightPath:
      'M80 89 Q86 85 92 89 M128 89 Q134 85 140 89',
    mouthPath: 'M101 120 Q110 125 119 120',
    leftPawClass: 'paw-work-left',
    rightPawClass: 'paw-work-right',
    shellClass: 'mode-working',
    tailClass: 'tail-working',
    earTone: 'rgba(202, 228, 255, 0.92)',
    cheekTone: 'rgba(191, 219, 254, 0.48)',
    cheekOpacity: 0.28,
  },
};
```

- [ ] **Step 2: 只在需要的状态里做细微表情差异，不要重画整张脸**

```js
const MODE_VARIANTS = {
  working: {
    编码中: {
      eyePath:
        'M69 96 Q79 82 95 92 Q90 102 69 96 Z M125 92 Q141 82 151 96 Q130 102 125 92 Z',
      eyeHighlightPath:
        'M80 89 Q85 86 90 89 M129 89 Q134 86 139 89',
      mouthPath: 'M102 120 Q110 123 118 120',
      cheekOpacity: 0.22,
    },
    沟通中: {
      eyePath:
        'M67 94 Q79 78 96 92 Q90 105 67 94 Z M124 92 Q141 78 153 94 Q130 105 124 92 Z',
      eyeHighlightPath:
        'M80 87 Q87 83 93 87 M128 87 Q135 83 141 87',
      mouthPath: 'M98 120 Q110 131 122 120',
      cheekOpacity: 0.36,
    },
  },
  reading: {
    调研中: {
      eyePath:
        'M68 97 Q79 83 94 94 Q88 104 68 97 Z M126 94 Q141 83 152 97 Q132 104 126 94 Z',
      eyeHighlightPath:
        'M81 90 Q86 87 91 89 M129 89 Q134 86 139 90',
      mouthPath: 'M101 121 Q110 124 119 121',
      cheekOpacity: 0.18,
    },
  },
};
```

- [ ] **Step 3: 更新状态测试，验证“统一底脸 + 小幅变化”成立**

```js
test('办公、阅读和待机应共享同一张大眼猫脸基线，只在细节上变化', () => {
  const idle = getAvatarModeMeta('idle');
  const coding = getAvatarModeMeta('working', '编码中');
  const research = getAvatarModeMeta('reading', '调研中');

  assert.match(idle.eyePath, /Z/);
  assert.equal(typeof idle.eyeHighlightPath, 'string');
  assert.notEqual(coding.eyePath, idle.eyePath);
  assert.notEqual(research.mouthPath, idle.mouthPath);
  assert.ok(coding.cheekOpacity < idle.cheekOpacity);
  assert.match(idle.eyePath, /M67 95/);
  assert.equal(idle.tailClass, 'tail-idle');
});
```

- [ ] **Step 4: 运行两组测试，确认脸部元数据与轮廓层一起通过**

Run: `node --test src/lib/components/Avatar/avatarOutline.test.js src/lib/components/Avatar/avatarStateMeta.test.js`

Expected: PASS，输出应包含两个测试文件的 `ok`，且不再出现 `eyeHighlightPath` 缺失或旧胡须直线断言。

- [ ] **Step 5: 提交状态脸部迁移**

```bash
git add src/lib/components/Avatar/avatarStateMeta.js src/lib/components/Avatar/avatarStateMeta.test.js src/lib/components/Avatar/avatarOutline.test.js
git commit -m "feat: refine avatar face expressions"
```

### Task 4: 做一次完整验证并人工检查关键状态

**Files:**
- Modify: `src/lib/components/Avatar/avatarOutline.test.js`（如测试断言需与最终路径微调同步）
- Modify: `src/lib/components/Avatar/avatarStateMeta.test.js`（如状态细节参数有最终微调）

- [ ] **Step 1: 跑完整的桌宠相关测试**

Run: `node --test src/lib/components/Avatar/avatarOutline.test.js src/lib/components/Avatar/avatarStateMeta.test.js src/lib/components/Avatar/avatarWindow.test.js`

Expected: PASS，三个测试文件全部通过，无 `ERR_TEST_FAILURE`。

- [ ] **Step 2: 启动本地前端并人工检查关键状态的脸部表现**

Run: `npm run dev`

Expected: Vite 正常启动，并能在桌宠窗口或对应页面里检查至少以下场景：
- `待机中`：眼睛灵动，不呆
- `编码中`：更专注，但脸不崩
- `调研中`：更收敛，仍然可爱
- `生成中`：认真但不苦脸
- 视觉总原则：接近已确认的 `A · 平衡版`，并且眼睛比预览稿 A 更大、更醒目

- [ ] **Step 3: 如发现最终观感问题，只允许收口以下类型的小修**

```txt
允许：
- 微调眼睛开合和高光位置
- 微调嘴角弧度
- 微调鼻口区上下位置
- 微调 cheekOpacity

不允许：
- 回退到旧的线条眼
- 重新引入横直胡须符号脸
- 临时修改窗口逻辑或 Rust 状态机掩盖视觉问题
```

- [ ] **Step 4: 运行最终回归测试**

Run: `node --test src/lib/components/Avatar/avatarOutline.test.js src/lib/components/Avatar/avatarStateMeta.test.js src/lib/components/Avatar/avatarWindow.test.js`

Expected: PASS，输出稳定，无新增失败。

- [ ] **Step 5: 提交最终脸部重设计**

```bash
git add src/lib/components/Avatar/avatarOutline.js src/lib/components/Avatar/AvatarCanvas.svelte src/lib/components/Avatar/avatarStateMeta.js src/lib/components/Avatar/avatarOutline.test.js src/lib/components/Avatar/avatarStateMeta.test.js
git commit -m "feat: redesign avatar face"
```
