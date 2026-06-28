import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import path from 'node:path';

const BASE_URL = process.env.README_CAPTURE_BASE_URL || 'http://127.0.0.1:5173';
const CAPTURE_DATE = '2026-06-28';
const VIEWPORT = { width: 1491, height: 841 };

const LOCALES = [
  { locale: 'zh-CN', dir: 'Introduction_zh' },
  { locale: 'en', dir: 'Introduction_en' },
  { locale: 'zh-TW', dir: 'Introduction_tw' },
];

const summariesByLocale = {
  'zh-CN': [
    {
      hour: 9,
      total_duration: 46 * 60,
      main_apps: 'Cursor, Chrome, Terminal',
      summary: '集中梳理 README 截图与多语言文案，核对界面预览、概览日期和时间线内容。同步检查截图尺寸规范，准备重新生成缺失页面。',
    },
    {
      hour: 10,
      total_duration: 58 * 60,
      main_apps: 'Cursor, Tauri Devtools, Chrome',
      summary: '排查时段摘要为空的原因，确认页面依赖 get_hourly_summaries 数据。补充代表性小时摘要，保证文档截图覆盖真实工作节奏。',
    },
    {
      hour: 14,
      total_duration: 37 * 60,
      main_apps: 'Chrome, Work Review, Notes',
      summary: '复核概览、时间线和日报页面的展示状态，重点检查应用类型、时间格式和语言切换是否一致。记录剩余文档规范项。',
    },
    {
      hour: 16,
      total_duration: 24 * 60,
      main_apps: 'Terminal, GitHub, Cursor',
      summary: '运行 README 测试与构建验证，整理截图资产并同步三语言 README。社区图、星标图和产品截图按用途保持稳定展示尺寸。',
    },
  ],
  en: [
    {
      hour: 9,
      total_duration: 46 * 60,
      main_apps: 'Cursor, Chrome, Terminal',
      summary: 'Reviewed README screenshots and multilingual copy, then checked preview pages, overview dates, and timeline content. Also verified image sizing rules before recapturing missing pages.',
    },
    {
      hour: 10,
      total_duration: 58 * 60,
      main_apps: 'Cursor, Tauri Devtools, Chrome',
      summary: 'Investigated why the hourly summary page was empty and confirmed that it depends on get_hourly_summaries data. Added representative hourly records so the documentation screenshot shows a real work rhythm.',
    },
    {
      hour: 14,
      total_duration: 37 * 60,
      main_apps: 'Chrome, Work Review, Notes',
      summary: 'Rechecked overview, timeline, and daily report screenshots, focusing on app categories, time formats, and language switching. Tracked the remaining documentation consistency items.',
    },
    {
      hour: 16,
      total_duration: 24 * 60,
      main_apps: 'Terminal, GitHub, Cursor',
      summary: 'Ran README tests and build verification, organized screenshot assets, and synced the three README languages. Community images, star history, and product screenshots now follow stable display sizes.',
    },
  ],
  'zh-TW': [
    {
      hour: 9,
      total_duration: 46 * 60,
      main_apps: 'Cursor, Chrome, Terminal',
      summary: '集中梳理 README 截圖與多語言文案，核對介面預覽、概覽日期和時間線內容。同步檢查截圖尺寸規範，準備重新生成缺失頁面。',
    },
    {
      hour: 10,
      total_duration: 58 * 60,
      main_apps: 'Cursor, Tauri Devtools, Chrome',
      summary: '排查時段摘要為空的原因，確認頁面依賴 get_hourly_summaries 資料。補充代表性小時摘要，確保文件截圖覆蓋真實工作節奏。',
    },
    {
      hour: 14,
      total_duration: 37 * 60,
      main_apps: 'Chrome, Work Review, Notes',
      summary: '複核概覽、時間線和日報頁面的展示狀態，重點檢查應用類型、時間格式和語言切換是否一致。記錄剩餘文件規範項。',
    },
    {
      hour: 16,
      total_duration: 24 * 60,
      main_apps: 'Terminal, GitHub, Cursor',
      summary: '執行 README 測試與構建驗證，整理截圖資產並同步三語言 README。社群圖、星標圖和產品截圖按用途保持穩定展示尺寸。',
    },
  ],
};

const defaultConfig = {
  theme: 'light',
  ui_visual_style: 'b',
  background_image: null,
  background_opacity: 0.25,
  background_blur: 1,
  lightweight_mode: false,
  work_end_hour: 18,
  work_end_minute: 0,
  work_time_segments: [
    { start_hour: 9, start_minute: 0, end_hour: 12, end_minute: 0 },
    { start_hour: 14, start_minute: 0, end_hour: 18, end_minute: 0 },
  ],
};

function createTauriMock(locale) {
  return {
    locale,
    captureDate: CAPTURE_DATE,
    summaries: summariesByLocale[locale],
    config: defaultConfig,
  };
}

async function main() {
  const browser = await chromium.launch({ headless: true });

  try {
    for (const item of LOCALES) {
      const outDir = path.join('docs', item.dir);
      await mkdir(outDir, { recursive: true });

      const context = await browser.newContext({
        viewport: VIEWPORT,
        deviceScaleFactor: 2,
        locale: item.locale,
      });

      await context.addInitScript((mock) => {
        window.localStorage.setItem('work-review.locale', mock.locale);
        const callbacks = new Map();

        function registerCallback(callback, once = false) {
          const id = Math.floor(Math.random() * Number.MAX_SAFE_INTEGER);
          callbacks.set(id, (data) => {
            if (once) callbacks.delete(id);
            return callback?.(data);
          });
          return id;
        }

        window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
          unregisterListener: () => {},
        };

        window.__TAURI_INTERNALS__ = {
          metadata: {
            currentWindow: { label: 'main' },
            currentWebview: { windowLabel: 'main', label: 'main' },
          },
          callbacks,
          transformCallback: registerCallback,
          unregisterCallback: (id) => callbacks.delete(id),
          runCallback: (id, data) => callbacks.get(id)?.(data),
          convertFileSrc: (filePath) => filePath,
          invoke: async (cmd, args = {}) => {
            switch (cmd) {
              case 'plugin:event|listen':
                return args.handler;
              case 'plugin:event|unlisten':
              case 'plugin:event|emit':
              case 'plugin:event|emit_to':
                return null;
              case 'get_platform':
              case 'get_runtime_platform':
                return 'macos';
              case 'get_config':
                return { ...mock.config };
              case 'save_config':
                return null;
              case 'get_recording_state':
                return [true, true];
              case 'get_background_image':
                return null;
              case 'get_today_stats':
              case 'get_overview_stats':
                return {
                  total_work_time: 4 * 3600 + 25 * 60,
                  app_usage: [],
                  browser_usage: [],
                  category_usage: [],
                  hourly_activity_distribution: [],
                };
              case 'get_timeline':
                return [];
              case 'get_hourly_summaries':
                return args.date === mock.captureDate ? mock.summaries : [];
              case 'get_saved_report':
                return null;
              case 'get_categories':
              case 'get_semantic_categories':
                return [];
              case 'should_check_updates':
                return false;
              default:
                return null;
            }
          },
        };
      }, createTauriMock(item.locale));

      const page = await context.newPage();
      await page.goto(`${BASE_URL}/#/timeline/summary/${CAPTURE_DATE}`, {
        waitUntil: 'networkidle',
      });
      await page.waitForSelector('.summary-band-card', { timeout: 15000 });
      await page.screenshot({
        path: path.join(outDir, '小时总结.png'),
        fullPage: false,
      });
      await context.close();
    }
  } finally {
    await browser.close();
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
