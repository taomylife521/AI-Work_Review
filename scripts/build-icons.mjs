import { mkdtemp, mkdir, rm, writeFile, copyFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import pngToIco from 'png-to-ico';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, '..');

const masterIcon = path.join(projectRoot, 'src-tauri', 'icons', 'icon.png');
const tauriIconsDir = path.join(projectRoot, 'src-tauri', 'icons');
const publicIconsDir = path.join(projectRoot, 'public', 'icons');
const hasFfmpeg = commandExists('ffmpeg');
const hasSips = commandExists('sips');

const persistentTargets = [
  { size: 16, output: path.join(tauriIconsDir, '16x16.png') },
  { size: 32, output: path.join(tauriIconsDir, '32x32.png') },
  { size: 48, output: path.join(tauriIconsDir, '48x48.png') },
  { size: 64, output: path.join(tauriIconsDir, '64x64.png') },
  { size: 128, output: path.join(tauriIconsDir, '128x128.png') },
  { size: 256, output: path.join(tauriIconsDir, '128x128@2x.png') },
  { size: 256, output: path.join(tauriIconsDir, '256x256.png') },
  { size: 512, output: path.join(tauriIconsDir, '512x512.png') },
  { size: 64, output: path.join(tauriIconsDir, 'tray-icon.png') },
  { size: 128, output: path.join(publicIconsDir, '128x128.png') },
  { size: 256, output: path.join(publicIconsDir, '256x256.png') },
];

const windowsIcoSizes = [16, 20, 24, 32, 40, 48, 64, 128, 256];

function commandExists(command) {
  const result = spawnSync(command, ['-version'], { stdio: 'ignore' });
  return result.status === 0;
}

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: projectRoot,
    encoding: 'utf8',
  });

  if (result.status !== 0) {
    const stderr = result.stderr?.trim();
    const stdout = result.stdout?.trim();
    throw new Error(
      [stderr, stdout].filter(Boolean).join('\n') || `${command} 执行失败`
    );
  }
}

function renderFilter(size) {
  if (size <= 24) {
    return `scale=${size}:${size}:flags=lanczos,unsharp=7:7:0.9:7:7:0.0`;
  }

  if (size <= 48) {
    return `scale=${size}:${size}:flags=lanczos,unsharp=5:5:0.75:5:5:0.0`;
  }

  if (size <= 64) {
    return `scale=${size}:${size}:flags=lanczos,unsharp=3:3:0.45:3:3:0.0`;
  }

  return `scale=${size}:${size}:flags=lanczos`;
}

async function buildPngWithFfmpeg(size, output) {
  run('ffmpeg', [
    '-y',
    '-i',
    masterIcon,
    '-vf',
    renderFilter(size),
    '-frames:v',
    '1',
    '-pix_fmt',
    'rgba',
    output,
  ]);
}

async function buildPngWithSips(size, output) {
  run('sips', ['-z', String(size), String(size), masterIcon, '--out', output]);
}

async function buildPng(size, output) {
  if (hasFfmpeg) {
    await buildPngWithFfmpeg(size, output);
    return;
  }

  if (hasSips) {
    await buildPngWithSips(size, output);
    return;
  }

  throw new Error('缺少 ffmpeg 或 sips，无法生成图标资源');
}

async function main() {
  if (!existsSync(masterIcon)) {
    throw new Error(`未找到主图标：${masterIcon}`);
  }

  await mkdir(publicIconsDir, { recursive: true });

  for (const target of persistentTargets) {
    await buildPng(target.size, target.output);
  }

  const tempDir = await mkdtemp(path.join(tmpdir(), 'work-review-icons-'));

  try {
    const icoPngs = [];

    for (const size of windowsIcoSizes) {
      const output = path.join(tempDir, `${size}.png`);
      await buildPng(size, output);
      icoPngs.push(output);
    }

    const ico = await pngToIco(icoPngs);
    await writeFile(path.join(tauriIconsDir, 'icon.ico'), ico);

    // 保持 public 下主图和 src-tauri 主图一致，避免应用内品牌图资源分叉
    await copyFile(masterIcon, path.join(projectRoot, 'public', 'icon.png'));

    console.log(
      `图标生成完成：Windows ICO 尺寸 ${windowsIcoSizes.join(', ')}`
    );
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
