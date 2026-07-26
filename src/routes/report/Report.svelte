<script>
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
  import { open } from '@tauri-apps/plugin-shell';
  import { marked } from 'marked';
  import DOMPurify from 'dompurify';
  import { showToast } from '../../lib/stores/toast.js';
  import { confirm } from '../../lib/stores/confirm.js';
  import CollapsibleSection from '../../lib/components/CollapsibleSection.svelte';
  import { cache } from '../../lib/stores/cache.js';
  import { formatLocalizedDate, formatLocalizedTime, formatDurationLocalized, locale, t, tm } from '$lib/i18n/index.js';
  import { formatUserError } from '$lib/utils/errorDisplay.js';
  import { shouldShowPromptAppliedToast } from './reportPromptFeedback.js';
  import { resolveReportMeta } from './reportMeta.js';
  import {
    extractReportBlockName,
    getVisibleReportSections,
    parseReportSections,
    reportSectionMarkdownForDisplay,
    reportSectionMarkdownForStorage,
  } from './reportSections.js';
  import LocalizedDatePicker from '../../lib/components/LocalizedDatePicker.svelte';

  function getLocalDateString() {
    const now = new Date();
    return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;
  }

  function getYesterdayDateString() {
    const yesterday = new Date();
    yesterday.setDate(yesterday.getDate() - 1);
    return `${yesterday.getFullYear()}-${String(yesterday.getMonth() + 1).padStart(2, '0')}-${String(yesterday.getDate()).padStart(2, '0')}`;
  }

  let report = null;
  let loading = false;
  let generating = false;
  let error = null;
  let selectedDate = getLocalDateString();
  let freshStats = null;
  let isYesterdayReport = false; // 标记是否显示的是昨日日报
  let showPresetModal = false;
  let presetSaving = false;
  $: activePresetName = (config?.daily_report_prompt_presets || []).find(p => p.prompt === config?.daily_report_custom_prompt)?.name || '';
  let editingPresetIndex = -1;
  let editingPresetName = '';
  let editingPresetPrompt = '';
  let config = null;
  let lastLoadedDate = '';
  let reportRequestId = 0;
  let exportInProgress = false;
  let promptSaving = false;
  let cacheData = null;
  const unsubscribeCache = cache.subscribe(v => {
    cacheData = v;
    // 首次或缓存有值时，立即从缓存恢复配置（避免页面切换闪烁）
    if (!config && v?.config) {
      config = v.config;
    }
  });
  onDestroy(unsubscribeCache);
  $: generating = cacheData?.reportGenerating ?? false;
  $: currentLocale = $locale;
  $: currentReportCacheKey = `${selectedDate}:${currentLocale}`;

  // 获取 AI 模式显示名称
  function getAiModeName(mode) {
    const normalizedMode = (mode || '').toString().trim().toLowerCase();
    const modeNames = {
      'local': t('report.modeNames.local'),
      'summary': t('report.modeNames.summary'),
      'cloud': t('report.modeNames.cloud')
    };
    return modeNames[normalizedMode] || mode || t('report.modeNames.unknown');
  }

  function getFallbackReasonText(meta) {
    return meta?.fallbackReason || t('report.savedReportNotAi');
  }

  async function loadConfig() {
    try {
      const cfg = await invoke('get_config');
      cache.setConfig(cfg);
    } catch (e) {
      console.error('加载配置失败:', e);
    }
  }

  async function loadReport(previousReport = null) {
    const requestId = ++reportRequestId;
    freshStats = null;

    // 并行加载实时统计
    invoke('get_daily_stats', { date: selectedDate })
      .then(stats => { if (requestId === reportRequestId) freshStats = stats; })
      .catch(() => {});

    // 乐观更新：先显示缓存数据
    let cacheData;
    const unsubscribe = cache.subscribe(c => { cacheData = c; });
    unsubscribe();
    
    if (cacheData.reports[currentReportCacheKey]?.data) {
      report = cacheData.reports[currentReportCacheKey].data;
      isYesterdayReport = false;
      loading = false;
      
      // 缓存有效则直接返回
      if (cache.isValid(cacheData.reports[currentReportCacheKey], 'reports')) {
        return;
      }
      
      // 后台静默刷新
      try {
        const savedReport = await invoke('get_saved_report', { date: selectedDate, locale: currentLocale });
        if (requestId !== reportRequestId) return;
        if (savedReport) {
          report = savedReport;
          cache.setReport(currentReportCacheKey, savedReport);
        }
      } catch (e) {
        console.warn('后台刷新日报失败:', e);
      }
    } else {
      // 首次加载
      loading = true;
      error = null;
      try {
        const savedReport = await invoke('get_saved_report', { date: selectedDate, locale: currentLocale });
        if (requestId !== reportRequestId) return;
        if (savedReport) {
          report = savedReport;
          isYesterdayReport = false;
          cache.setReport(currentReportCacheKey, savedReport);
        } else {
          if (!savedReport && previousReport?.date === selectedDate && previousReport?.content) {
            cache.setReportGenerating(true);
            await invoke('generate_report', { date: selectedDate, force: false, locale: currentLocale });
            const localizedReport = await invoke('get_saved_report', { date: selectedDate, locale: currentLocale });

            if (localizedReport) {
              report = localizedReport;
              isYesterdayReport = false;
              cache.setReport(currentReportCacheKey, localizedReport);
              return;
            }
          }

          // 如果选择今天且今天无日报，尝试加载昨日日报
          if (selectedDate === getLocalDateString()) {
            const yesterday = getYesterdayDateString();
            const yesterdayReport = await invoke('get_saved_report', { date: yesterday, locale: currentLocale });
            if (yesterdayReport) {
              report = yesterdayReport;
              isYesterdayReport = true;
            } else {
              report = null;
              isYesterdayReport = false;
            }
          } else {
             report = null;
             isYesterdayReport = false;
          }
        }
      } catch (e) {
        error = formatUserError(e, t('common.loadFailedRetry'));
      } finally {
        cache.setReportGenerating(false);
        loading = false;
      }
    }
  }

  function selectDate(date) {
    if (!date || date === selectedDate) return;
    selectedDate = date;
  }

  async function generateReport(force = true) {
    cache.setReportGenerating(true);
    error = null;
    try {
      if (config?.ai_mode === 'summary') {
        await persistReportPrompt();
      }
      await invoke('generate_report', { date: selectedDate, force, locale: currentLocale });
      const savedReport = await invoke('get_saved_report', { date: selectedDate, locale: currentLocale });
      report = savedReport || { date: selectedDate, content: '', created_at: Date.now() / 1000 };
      isYesterdayReport = false;
      cache.setReport(currentReportCacheKey, report);

      if (
        shouldShowPromptAppliedToast({
          configAiMode: config?.ai_mode,
          customPrompt: config?.daily_report_custom_prompt,
          reportAiMode: savedReport?.ai_mode,
        })
      ) {
        showToast(t('report.promptApplied'), 'success');
      }
    } catch (e) {
      error = formatUserError(e, t('common.loadFailedRetry'));
    } finally {
      cache.setReportGenerating(false);
    }
  }

  async function persistReportPrompt() {
    if (!config || config.ai_mode !== 'summary' || promptSaving) {
      return;
    }

    promptSaving = true;
    try {
      config.daily_report_custom_prompt = (config.daily_report_custom_prompt || '').trim();
      await invoke('save_config', { config });
    } finally {
      promptSaving = false;
    }
  }

  /** 预设数量上限：防止胶片区无界增高（与工作时段 MAX_WORK_SEGMENTS 同类防御）。 */
  const MAX_PROMPT_PRESETS = 12;

  async function savePresets() {
    try {
      await invoke('save_config', { config });
    } catch (e) {
      console.error('保存预设失败:', e);
    }
  }

  // 把节点移到 document.body，规避祖先的 backdrop-filter / overflow 对 position:fixed 的干扰
  async function exportReportMarkdown() {
    if (!report) return;

    exportInProgress = true;
    try {
      let exportDir = config?.daily_report_export_dir || null;
      if (!exportDir) {
        const selected = await openDialog({
          directory: true,
          multiple: false,
        });

        if (!selected || Array.isArray(selected)) {
          return;
        }

        exportDir = selected;
      }

      const exportPath = await invoke('export_report_markdown', {
        date: report.date || selectedDate,
        content: report.content,
        exportDir,
      });
      showToast(t('report.exportSuccess', { path: exportPath }), 'success');
    } catch (e) {
      showToast(t('report.exportFailed', { error: e }), 'error');
    } finally {
      exportInProgress = false;
    }
  }

  // ===== 批量日报合并导出 =====
  let showBatchExportModal = false;
  let batchExporting = false;
  let batchStartDate = '';
  let batchEndDate = '';

  // ISO 日期字符串工具（避开 toISOString 的 UTC 时区坑）
  function toIsoDate(date) {
    const y = date.getFullYear();
    const m = String(date.getMonth() + 1).padStart(2, '0');
    const d = String(date.getDate()).padStart(2, '0');
    return `${y}-${m}-${d}`;
  }

  // 计算"本周/上周"的范围，约定周一为一周开始
  // 注：getDay() 周日=0，周一=1，所以 (day + 6) % 7 是距离本周一的天数
  function weekRange(offsetWeeks) {
    const today = new Date();
    const dayFromMonday = (today.getDay() + 6) % 7;
    const monday = new Date(today);
    monday.setDate(today.getDate() - dayFromMonday + offsetWeeks * 7);
    const sunday = new Date(monday);
    sunday.setDate(monday.getDate() + 6);
    return { start: toIsoDate(monday), end: toIsoDate(sunday) };
  }

  function monthRange(offsetMonths) {
    const today = new Date();
    const start = new Date(today.getFullYear(), today.getMonth() + offsetMonths, 1);
    const end = new Date(today.getFullYear(), today.getMonth() + offsetMonths + 1, 0);
    return { start: toIsoDate(start), end: toIsoDate(end) };
  }

  function applyBatchPreset(preset) {
    let range;
    if (preset === 'thisWeek') range = weekRange(0);
    else if (preset === 'lastWeek') range = weekRange(-1);
    else if (preset === 'thisMonth') range = monthRange(0);
    else if (preset === 'lastMonth') range = monthRange(-1);
    if (range) {
      batchStartDate = range.start;
      batchEndDate = range.end;
    }
  }

  function openBatchExportModal() {
    // 默认填本月范围，省一步点击
    if (!batchStartDate || !batchEndDate) {
      applyBatchPreset('thisMonth');
    }
    showBatchExportModal = true;
  }

  async function exportReportsRange() {
    if (batchExporting) return;
    if (!batchStartDate || !batchEndDate) {
      showToast(t('report.batchExportInvalidRange'), 'error');
      return;
    }
    if (batchStartDate > batchEndDate) {
      showToast(t('report.batchExportInvalidRange'), 'error');
      return;
    }

    const targetPath = await saveDialog({
      defaultPath: `reports-${batchStartDate}_to_${batchEndDate}.md`,
      filters: [{ name: 'Markdown', extensions: ['md'] }],
    });
    if (!targetPath) return;

    batchExporting = true;
    try {
      const result = await invoke('export_reports_range', {
        startDate: batchStartDate,
        endDate: batchEndDate,
        targetPath,
        locale: currentLocale,
      });
      showToast(
        t('report.batchExportSuccess', { path: result.path, count: result.count }),
        'success',
      );
      showBatchExportModal = false;
    } catch (e) {
      showToast(t('report.batchExportFailed', { error: e }), 'error');
    } finally {
      batchExporting = false;
    }
  }

  function renderMarkdown(content) {
    const rawHtml = marked(content);
    return DOMPurify.sanitize(rawHtml);
  }

  async function handleReportLinkClick(event) {
    const link = event.target.closest('a[href]');
    if (!link) return;

    const href = link.getAttribute('href');
    if (!href || href.startsWith('#')) return;

    event.preventDefault();
    try {
      await open(href);
    } catch (e) {
      console.error('打开日报链接失败:', e);
    }
  }

  function interceptReportLinks(node) {
    const listener = (event) => {
      handleReportLinkClick(event);
    };

    node.addEventListener('click', listener);

    return {
      destroy() {
        node.removeEventListener('click', listener);
      }
    };
  }

  // 结构化编辑：将 markdown 按 ## 标题拆分为段落
  let editingSection = -1; // 当前正在编辑的段落索引
  let editingContent = ''; // 编辑中的内容
  let showBlockManager = false; // 段落管理弹层

  function startEditSection(sections, index) {
    editingSection = index;
    const section = sections[index];
    editingContent = reportSectionMarkdownForStorage(section);
  }

  /** 删除预设：走全局确认弹窗(与全应用删除交互一致);删除当前生效预设时清空提示词。 */
  async function deletePreset(index) {
    const preset = (config?.daily_report_prompt_presets || [])[index];
    if (!preset) return;
    const ok = await confirm({
      tone: 'warning',
      title: t('report.confirmDeletePreset', { name: preset.name }),
      message: preset.prompt,
      confirmText: t('common.confirm'),
      cancelText: t('common.cancel'),
    });
    if (!ok) return;
    const wasActive = config.daily_report_custom_prompt === preset.prompt;
    config.daily_report_prompt_presets = config.daily_report_prompt_presets.filter((_, j) => j !== index);
    if (wasActive) {
      config.daily_report_custom_prompt = '';
      persistReportPrompt();
    }
    await savePresets();
  }

  function cancelEditSection() {
    editingSection = -1;
    editingContent = '';
  }

  let savingSection = false;
  async function saveEditSection(sections, index) {
    if (savingSection) return;
    savingSection = true;
    const newContent = editingContent.trim();
    const newSections = [...sections];
    const parsed = parseReportSections(newContent || '');
    if (parsed.length > 0) {
      newSections[index] = parsed[0];
      // If user added more ## headers, merge them in
      if (parsed.length > 1) {
        newSections.splice(index + 1, 0, ...parsed.slice(1));
      }
    }

    const fullContent = newSections.map(reportSectionMarkdownForStorage).join('\n');

    try {
      await invoke('update_report_content', { date: selectedDate, locale: currentLocale, content: fullContent });
      report = { ...report, content: fullContent };
      cache.setReport(currentReportCacheKey, report);
      editingSection = -1;
      editingContent = '';
    } catch (e) {
      showToast(t('report.editSectionFailed') + ': ' + e, 'error');
    } finally {
      savingSection = false;
    }
  }

  function formatReportDate(dateStr) {
    // 用正午时间避免 "YYYY-MM-DD" 被按 UTC 午夜解析导致西时区日期偏移一天
    const date = new Date(`${dateStr}T12:00:00`);
    return formatLocalizedDate(date, { year: 'numeric', month: 'long', day: 'numeric', weekday: 'long' });
  }

  $: if (currentReportCacheKey && currentReportCacheKey !== lastLoadedDate) {
    const previousReport = report;
    lastLoadedDate = currentReportCacheKey;
    report = null;
    editingSection = -1;
    isYesterdayReport = false;
    loadReport(previousReport);
  }

  $: reportSections = parseReportSections(report?.content || '');
  // 钉选/隐藏偏好（从 config 读取，前端即时过滤）
  $: pinnedBlocks = config?.daily_report_pinned_blocks || [];
  $: hiddenBlocks = config?.daily_report_hidden_blocks || [];

  $: visibleSections = getVisibleReportSections(reportSections, pinnedBlocks, hiddenBlocks);

  async function togglePinBlock(section) {
    const blockName = extractReportBlockName(section);
    if (!blockName) return;
    const newPinned = pinnedBlocks.includes(blockName)
      ? pinnedBlocks.filter((b) => b !== blockName)
      : [...pinnedBlocks, blockName];
    try {
      await invoke('set_report_block_preference', {
        pinnedBlocks: newPinned,
        hiddenBlocks,
      });
      config = { ...config, daily_report_pinned_blocks: newPinned };
    } catch (e) { console.error('设置钉选失败:', e); }
  }

  async function toggleHideBlock(section) {
    const blockName = extractReportBlockName(section);
    if (!blockName) return;
    const newHidden = hiddenBlocks.includes(blockName)
      ? hiddenBlocks.filter((b) => b !== blockName)
      : [...hiddenBlocks, blockName];
    try {
      await invoke('set_report_block_preference', {
        pinnedBlocks,
        hiddenBlocks: newHidden,
      });
      config = { ...config, daily_report_hidden_blocks: newHidden };
    } catch (e) { console.error('设置隐藏失败:', e); }
  }

  $: reportMeta = resolveReportMeta(report, config);

  // ══════════ 段落目录（长日报导航,xl 屏显示） ══════════
  let activeSectionIndex = 0;
  let sectionObserver = null;

  function tocTitle(section) {
    return (section?.title || '').replace(/^#+\s*/, '').trim();
  }

  /** 段落锚点 action:注册到 IntersectionObserver,滚动时高亮当前段。 */
  function tocAnchor(node, index) {
    node.dataset.tocIndex = String(index);
    if (!sectionObserver && typeof IntersectionObserver !== 'undefined') {
      sectionObserver = new IntersectionObserver(
        (entries) => {
          for (const entry of entries) {
            if (entry.isIntersecting) {
              activeSectionIndex = Number(entry.target.dataset.tocIndex) || 0;
            }
          }
        },
        { rootMargin: '-20% 0px -70% 0px' }
      );
    }
    sectionObserver?.observe(node);
    return {
      update(nextIndex) {
        node.dataset.tocIndex = String(nextIndex);
      },
      destroy() {
        sectionObserver?.unobserve(node);
      },
    };
  }

  function scrollToSection(index) {
    document
      .getElementById(`report-sec-${index}`)
      ?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }

  onMount(() => {
    loadConfig();
    return () => {
      sectionObserver?.disconnect();
      sectionObserver = null;
    };
  });

  // 页面重新获得焦点时刷新配置，确保 AI 增强状态为最新
  let configRefreshTimer = 0;
  function refreshConfigOnFocus() {
    const now = Date.now();
    if (now - configRefreshTimer < 2000) return;
    configRefreshTimer = now;
    loadConfig();
  }
</script>

<svelte:window on:focusin={refreshConfigOnFocus} on:visibilitychange={() => {
  if (document.visibilityState === 'visible') refreshConfigOnFocus();
}} />

<div class="page-shell report-editorial-shell" data-locale={currentLocale}>
  <!-- 页面标题 -->
  <div class="report-hero">
    <div class="report-hero-main">
      <div class="page-title-group report-hero-copy">
      <div class="page-title-badge">
        <svg fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M8 7h8M8 12h8M8 17h5" />
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M7 3h7l5 5v10a3 3 0 01-3 3H7a3 3 0 01-3-3V6a3 3 0 013-3Z" />
        </svg>
      </div>
      <div class="page-title-copy">
        <h2>
          {selectedDate === getLocalDateString() ? t('report.todayReport') : t('report.historyReport')}
        </h2>
        <div class="report-hero-meta">
          <div class="report-hero-date-row">
            <span class="report-hero-date">{formatReportDate(selectedDate)}</span>
            {#if config || report}
              <span class="report-hero-mode-chip">{getAiModeName(reportMeta.reportMode)}</span>
            {/if}
          </div>
          {#if config || report}
            {#if reportMeta.showUsageMismatchNotice}
              <p class="report-hero-mode-note">{t('report.aiNotAppliedPrefix')}{getFallbackReasonText(reportMeta)}</p>
            {/if}
          {/if}
        </div>
      </div>
    </div>
      <div class="report-hero-actions">
      <div class="page-toolbar-end">
        <button
          class="page-control-btn {selectedDate === getLocalDateString() ? 'page-control-btn-active' : ''}"
          on:click={() => selectDate(getLocalDateString())}
        >
          {t('report.today')}
        </button>
        <button
          class="page-control-btn {selectedDate === getYesterdayDateString() ? 'page-control-btn-active' : ''}"
          on:click={() => selectDate(getYesterdayDateString())}
        >
          {t('report.yesterday')}
        </button>
        {#key `report-date-${currentLocale}`}
          <LocalizedDatePicker
            bind:value={selectedDate}
            max={getLocalDateString()}
            localeCode={currentLocale}
            triggerClass="page-control-input w-auto"
          />
        {/key}
      </div>
      <div class="flex flex-wrap justify-end gap-2">
        {#if report}
          <button
            class="page-action-secondary min-h-10 px-4 py-2"
            on:click={exportReportMarkdown}
            disabled={exportInProgress}
            title={config?.daily_report_export_dir ? '' : t('report.exportWithoutDefaultDir')}
          >
            {#if exportInProgress}
              <div class="animate-spin rounded-full h-4 w-4 border-2 border-current border-t-transparent"></div>
              {t('report.exporting')}
            {:else}
              {t('report.exportMarkdown')}
            {/if}
          </button>
          <button
            class="page-action-secondary min-h-10 px-4 py-2"
            on:click={openBatchExportModal}
            disabled={batchExporting}
            title={t('report.batchExportTitle')}
          >
            {t('report.batchExport')}
          </button>
          {#if hiddenBlocks.length > 0}
            <button
              class="page-action-secondary min-h-10 px-4 py-2"
              on:click={() => (showBlockManager = !showBlockManager)}
              title={t('report.manageBlocks')}
            >
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 10h16M4 14h16M4 18h16" />
              </svg>
              {t('report.manageBlocks')}
              <span class="ml-1 rounded-full bg-slate-200 dark:bg-[#484f58] px-1.5 text-[10px] font-semibold">{hiddenBlocks.length}</span>
            </button>
          {/if}
          <button
            class="page-action-warn"
            on:click={() => generateReport(true)}
            disabled={generating}
          >
            {#if generating}
              <div class="animate-spin rounded-full h-4 w-4 border-2 border-white border-t-transparent"></div>
              {t('report.generating')}
            {:else}
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
              </svg>
              {t('report.regenerate')}
            {/if}
          </button>
        {/if}
      </div>
    </div>
    </div>
  </div>

  <div class="report-editorial-stack">
  {#if config && config.ai_mode === 'summary'}
    <CollapsibleSection title={t('report.promptSettings')} storageKey="report.promptSettings">
      <label for="daily-report-custom-prompt" class="settings-label mb-1.5">{t('report.promptLabel')}</label>

      <!-- 预设胶片集合：与分类管理同一套交互语言(点选应用,悬停浮出编辑/删除角标) -->
      <div class="mb-2 flex flex-wrap gap-2">
        {#each (config?.daily_report_prompt_presets || []) as preset, i}
          {@const presetActive = config.daily_report_custom_prompt === preset.prompt}
          <div class="group/preset relative">
            <button
              type="button"
              class="segment-btn flex-none rounded-lg border px-3 py-1.5 text-xs max-w-56 truncate
                {presetActive ? 'settings-segment-success' : 'settings-segment-idle'}"
              title={presetActive ? t('report.presetClickToUnselect') : preset.prompt}
              on:click={() => {
                // 再次点击已选中的预设 = 取消使用（回到"不带附加提示词"状态）
                config.daily_report_custom_prompt = presetActive ? '' : preset.prompt;
                persistReportPrompt();
              }}
            >
              {preset.name}
            </button>
            <button
              type="button"
              class="absolute -top-1.5 left-1/2 -translate-x-1/2 flex h-5 w-5 items-center justify-center rounded-full bg-blue-500 text-xs leading-none text-white opacity-0 shadow-sm transition-opacity hover:bg-blue-600 group-hover/preset:opacity-100 focus-visible:opacity-100 dark:shadow-none"
              title={t('report.editPreset')}
              on:click|stopPropagation={() => {
                editingPresetIndex = i;
                editingPresetName = preset.name;
                editingPresetPrompt = preset.prompt;
                showPresetModal = true;
              }}
            >✎</button>
            <button
              type="button"
              class="absolute -top-1.5 -end-1.5 flex h-5 w-5 items-center justify-center rounded-full bg-red-500 text-xs leading-none text-white opacity-0 shadow-sm transition-opacity hover:bg-red-600 group-hover/preset:opacity-100 focus-visible:opacity-100 dark:shadow-none"
              title={t('common.delete')}
              on:click|stopPropagation={() => deletePreset(i)}
            >×</button>
          </div>
        {/each}
        {#if (config?.daily_report_prompt_presets || []).length < MAX_PROMPT_PRESETS}
          <button
            type="button"
            class="segment-btn settings-segment-idle flex-none rounded-lg border border-dashed px-3 py-1.5 text-xs"
            on:click={() => {
              editingPresetIndex = -1;
              editingPresetName = '';
              editingPresetPrompt = '';
              showPresetModal = true;
            }}
          >
            + {t('report.addPreset')}
          </button>
        {:else}
          <span class="inline-flex items-center px-2 text-xs text-slate-400 dark:text-[#636c76]" title={t('report.presetLimitReached', { max: MAX_PROMPT_PRESETS })}>
            {t('report.presetLimitReached', { max: MAX_PROMPT_PRESETS })}
          </span>
        {/if}
      </div>
      <textarea
        id="daily-report-custom-prompt"
        bind:value={config.daily_report_custom_prompt}
        on:change={persistReportPrompt}
        rows="3"
        class="control-input resize-y min-h-[80px]"
        placeholder={t('report.promptPlaceholder')}
      ></textarea>

      <!-- 系统提示词覆盖 -->
      <div class="mt-4 pt-3 border-t border-slate-200 dark:border-[#30363d]">
        <div class="flex items-center justify-between mb-2">
          <label for="daily-report-system-prompt-override" class="text-sm font-medium text-slate-700 dark:text-[#adbac7]">
            {t('report.systemPromptOverride')}
          </label>
          <button
            type="button"
            class="text-xs text-slate-400 hover:text-slate-700 dark:hover:text-[#adbac7] transition"
            on:click={() => { config.daily_report_system_prompt_override = null; }}
            disabled={!config.daily_report_system_prompt_override}
          >
            {t('report.resetSystemPrompt')}
          </button>
        </div>
        <p class="text-xs text-slate-400 dark:text-[#636c76] mb-2">{t('report.systemPromptOverrideHint')}</p>
        <textarea
          id="daily-report-system-prompt-override"
          rows="6"
          class="control-input resize-y min-h-[100px] font-mono text-xs"
          bind:value={config.daily_report_system_prompt_override}
          on:change={persistReportPrompt}
          placeholder={t('report.systemPromptOverridePlaceholder')}
        ></textarea>
      </div>
    </CollapsibleSection>
  {/if}

  <!-- 日报内容 -->
  {#if loading}
    <div class="empty-state-lg">
      <div class="empty-state-icon">
        <div class="animate-spin rounded-full h-8 w-8 border-2 border-indigo-500 border-t-transparent"></div>
      </div>
      <h3 class="empty-state-title">{t('report.loadingTitle')}</h3>
      <p class="empty-state-copy mt-1">{t('report.loadingCopy')}</p>
    </div>
  {:else if error}
    <div class="page-banner-error">
      <div>
        <div class="flex items-center gap-3 text-red-500 mb-2">
        <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
        </svg>
        <span class="font-medium">{t('report.generateFailed')}</span>
      </div>
      <p class="text-sm">{error}</p>
      </div>
      <button class="page-action-brand" on:click={() => generateReport(true)}>{t('common.retry')}</button>
    </div>
  {:else if report}
    <!-- 昨日日报提示 -->
    {#if isYesterdayReport}
      <div class="page-banner-warning report-fallback-banner mb-4">
        <div class="report-fallback-copy">
          <div class="flex items-center gap-2 text-sm">
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          {t('report.showingYesterday', { date: formatReportDate(report.date) })}
          </div>
        </div>
        <div class="report-fallback-action">
          <button
            class="page-action-warn report-fallback-button min-h-9 px-3 text-xs rounded-lg shadow-none"
            on:click={() => generateReport(false)}
            disabled={generating}
          >
            {#if generating}
              <div class="inline-flex items-center gap-2">
                <div class="animate-spin rounded-full h-3 w-3 border-2 border-white border-t-transparent"></div>
                <span>{t('report.generating')}</span>
              </div>
            {:else}
              ✨ {t('report.generatingToday')}
            {/if}
          </button>
        </div>
      </div>
    {/if}
    {#if showBlockManager && hiddenBlocks.length > 0}
      <div class="page-card mb-4 p-4">
        <div class="flex items-center justify-between mb-3">
          <h3 class="text-sm font-semibold">{t('report.manageBlocksTitle')}</h3>
          <button class="text-slate-400 hover:text-slate-600 dark:text-[#7d8590] dark:hover:text-[#c9d1d9]" on:click={() => (showBlockManager = false)}>
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" /></svg>
          </button>
        </div>
        <div class="flex flex-wrap gap-2">
          {#each hiddenBlocks as blockName}
            <button
              class="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 dark:border-[#30363d] bg-white dark:bg-[#21262d] px-3 py-1.5 text-xs font-medium text-slate-700 dark:text-[#adbac7] hover:border-indigo-300 dark:hover:border-indigo-500 transition-colors"
              on:click={async () => {
                const newHidden = hiddenBlocks.filter((b) => b !== blockName);
                try {
                  await invoke('set_report_block_preference', { pinnedBlocks, hiddenBlocks: newHidden });
                  config = { ...config, daily_report_hidden_blocks: newHidden };
                } catch (e) { console.error('设置隐藏失败:', e); }
              }}
            >
              <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" /></svg>
              {tm(`report.blockNames.${blockName}`) || blockName}
            </button>
          {/each}
        </div>
      </div>
    {/if}
    <div class="report-reading-layout">
      <!-- 段落目录：长日报导航（xl 屏显示,点击平滑滚动,滚动时高亮当前段） -->
      {#if visibleSections.length > 1}
        <nav class="report-toc" aria-label={t('report.tocLabel')}>
          <p class="report-toc-title">{t('report.tocLabel')}</p>
          <ul>
            {#each visibleSections as section, i}
              {@const label = tocTitle(section)}
              {#if label}
                <li>
                  <button
                    type="button"
                    class="report-toc-item {activeSectionIndex === i ? 'report-toc-item-active' : ''}"
                    on:click={() => scrollToSection(i)}
                  >
                    {label}
                  </button>
                </li>
              {/if}
            {/each}
          </ul>
        </nav>
      {/if}

      <div class="page-card report-sheet report-article-card min-w-0">
      <div class="report-sheet-content">
        <!-- 昨日日报醒目提示：避免被误认为今天的 -->
        {#if isYesterdayReport}
          <div class="mb-4 flex flex-wrap items-center justify-between gap-2 rounded-xl border border-amber-200/80 bg-amber-50/80 px-3.5 py-2.5 dark:border-amber-500/30 dark:bg-amber-950/25">
            <p class="text-sm text-amber-800 dark:text-amber-200">{t('report.yesterdayBanner')}</p>
            <button
              class="shrink-0 rounded-lg bg-amber-500 px-3 py-1.5 text-xs font-medium text-white transition hover:bg-amber-600 disabled:opacity-50"
              on:click={() => generateReport(false)}
              disabled={generating}
            >
              {generating ? t('report.generating') : t('report.generateTodayNow')}
            </button>
          </div>
        {/if}
        <div class="report-sheet-meta text-xs text-slate-400 dark:text-[#7d8590] mb-4 flex items-center gap-2">
          <div class="w-1.5 h-1.5 rounded-full {isYesterdayReport ? 'bg-amber-500' : 'bg-emerald-500'}"></div>
          {isYesterdayReport ? t('report.yesterdayPrefix') : ''}{t('report.generatedAt', { time: formatLocalizedDate(new Date(report.created_at * 1000), { year: 'numeric', month: '2-digit', day: '2-digit' }) + ' ' + formatLocalizedTime(new Date(report.created_at * 1000), { hour: '2-digit', minute: '2-digit', second: '2-digit' }) })}
        </div>
        {#if freshStats}
          <div class="grid grid-cols-2 sm:grid-cols-4 gap-3 mb-8">
            <div class="report-stat-card">
              <div class="report-stat-label">{t('report.statTotalDuration')}</div>
              <div class="report-stat-value">{formatDurationLocalized(freshStats.total_duration)}</div>
            </div>
            <div class="report-stat-card">
              <div class="report-stat-label">{t('report.statScreenshots')}</div>
              <div class="report-stat-value">{freshStats.screenshot_count}</div>
            </div>
            <div class="report-stat-card">
              <div class="report-stat-label">{t('report.statApps')}</div>
              <div class="report-stat-value">{freshStats.app_usage?.length ?? 0}</div>
            </div>
            <div class="report-stat-card">
              <div class="report-stat-label">{t('report.statWebsites')}</div>
              <div class="report-stat-value">{freshStats.domain_usage?.length ?? 0}</div>
            </div>
          </div>
        {/if}
        <div class="markdown-body report-sheet-body prose prose-slate dark:prose-invert max-w-none">
          {#each visibleSections as section, i}
            {@const blockName = extractReportBlockName(section)}
            <div class="report-section group/section" id={`report-sec-${i}`} use:tocAnchor={i}>
              <div class="report-section-header">
                <div
                  use:interceptReportLinks
                  class="report-section-content"
                >
                  {@html renderMarkdown(reportSectionMarkdownForDisplay(section, section.displaySectionIndex ?? i, currentLocale))}
                </div>
                <div class="report-section-actions flex items-center gap-1 opacity-0 group-hover/section:opacity-100 focus-within:opacity-100 transition-opacity">
                  {#if blockName}
                    <button
                      class="report-section-edit-btn"
                      on:click={() => togglePinBlock(section)}
                      title={pinnedBlocks.includes(blockName) ? t('report.unpinBlock') : t('report.pinBlock')}
                    >
                      <svg class="w-3.5 h-3.5" fill={pinnedBlocks.includes(blockName) ? 'currentColor' : 'none'} stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
                      </svg>
                    </button>
                    <button
                      class="report-section-edit-btn"
                      on:click={() => toggleHideBlock(section)}
                      title={t('report.hideBlock')}
                    >
                      <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.578 7.578l3.29 3.29M3 3l3.59 3.59m0 0A9.953 9.953 0 0112 5c4.478 0 8.268 2.943 9.543 7a10.025 10.025 0 01-4.132 5.411m0 0L21 21" />
                      </svg>
                    </button>
                  {/if}
                  <button
                    class="report-section-edit-btn"
                    on:click={() => startEditSection(reportSections, section.originalIndex ?? i)}
                    title={t('report.editSection')}
                  >
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z" />
                    </svg>
                  </button>
                </div>
              </div>
            </div>
          {/each}
        </div>
      </div>
      </div>
    </div>
    {:else if generating}
    <!-- 生成中骨架屏：替代空白等待 -->
    <div class="page-card report-sheet report-article-card">
      <div class="report-sheet-content animate-pulse space-y-4 py-2">
        <div class="h-3 w-40 rounded-full bg-slate-200/80 dark:bg-[#21262d]"></div>
        <div class="grid grid-cols-2 gap-3 sm:grid-cols-4">
          {#each Array(4) as _}
            <div class="h-16 rounded-lg bg-slate-100/90 dark:bg-[#161b22]"></div>
          {/each}
        </div>
        <div class="h-6 w-1/3 rounded-full bg-slate-200/80 dark:bg-[#21262d]"></div>
        <div class="space-y-2.5">
          {#each Array(3) as _}
            <div class="h-3.5 rounded-full bg-slate-100/90 dark:bg-[#161b22]"></div>
          {/each}
          <div class="h-3.5 w-2/3 rounded-full bg-slate-100/90 dark:bg-[#161b22]"></div>
        </div>
        <div class="h-6 w-1/4 rounded-full bg-slate-200/80 dark:bg-[#21262d]"></div>
        <div class="space-y-2.5">
          {#each Array(4) as _}
            <div class="h-3.5 rounded-full bg-slate-100/90 dark:bg-[#161b22]"></div>
          {/each}
        </div>
        <p class="pt-2 text-center text-xs text-slate-400 dark:text-[#636c76]">{t('report.generating')}…</p>
      </div>
    </div>
    {:else}
    <div class="empty-state-lg">
      <div class="empty-state-icon !w-16 !h-16 !mb-5 bg-amber-50 dark:bg-amber-950/30">
        <span class="text-3xl">📝</span>
      </div>
      <h3 class="empty-state-title">
        {selectedDate === getLocalDateString() ? t('report.noReportToday') : t('report.noReportForDate', { date: selectedDate })}
      </h3>
      <p class="empty-state-copy mb-5">
        {t('report.aiWillGenerate')}
      </p>
      <button
        class="page-action-warn min-h-11 px-6 py-3"
        on:click={() => generateReport(false)}
        disabled={generating}
      >
        {#if generating}
          <div class="inline-flex items-center gap-2">
            <div class="animate-spin rounded-full h-4 w-4 border-2 border-white border-t-transparent"></div>
            {t('report.generating')}
          </div>
        {:else}
          ✨ {selectedDate === getLocalDateString() ? t('report.generatingToday') : t('report.generatingSelected')}
        {/if}
      </button>
    </div>
  {/if}
</div>
</div>

<!-- 段落编辑弹窗 -->
{#if editingSection >= 0}
  <div class="modal-overlay" role="dialog" aria-modal="true" aria-labelledby="report-edit-section-title">
    <button type="button" class="absolute inset-0 cursor-default" aria-label={t('report.cancelEdit')} on:click={cancelEditSection}></button>
    <div class="modal-panel relative z-10">
      <div class="modal-header">
        <h3 id="report-edit-section-title" class="modal-title">{t('report.editSection')}</h3>
        <button class="modal-close" on:click={cancelEditSection}>
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>
      <div class="modal-body">
        <textarea
          class="report-edit-textarea"
          bind:value={editingContent}
        ></textarea>
      </div>
      <div class="modal-footer">
        <button class="page-control-btn" on:click={cancelEditSection}>
          {t('report.cancelEdit')}
        </button>
        <button
          class="page-action-brand"
          on:click={() => saveEditSection(reportSections, editingSection)}
          disabled={savingSection}
        >
          {#if savingSection}
            <div class="inline-flex items-center gap-2">
              <div class="animate-spin rounded-full h-4 w-4 border-2 border-white border-t-transparent"></div>
              {t('report.saveSection')}
            </div>
          {:else}
            {t('report.saveSection')}
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- 表格 / 标题 / 列表等 markdown 样式已统一放到 app.css .markdown-body -->

{#if showPresetModal}
  <div class="modal-overlay" role="dialog" aria-modal="true" aria-labelledby="report-preset-dialog-title">
    <button type="button" class="absolute inset-0 cursor-default" aria-label={t('report.cancelEdit')} on:click={() => { showPresetModal = false; }}></button>
    <div class="modal-panel relative z-10" style="max-width: 36rem;">
      <div class="modal-header">
        <h3 id="report-preset-dialog-title" class="modal-title">{editingPresetIndex >= 0 ? editingPresetName || t('report.presetsTitle') : t('report.addPreset')}</h3>
        <button class="modal-close" on:click={() => { showPresetModal = false; }}>
          <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
        </button>
      </div>
      <div class="modal-body space-y-4">
        <div>
          <label for="report-preset-name" class="block text-xs font-medium text-slate-500 dark:text-[#7d8590] mb-1.5">{t('report.presetNamePlaceholder')}</label>
          <input
            id="report-preset-name"
            type="text"
            class="w-full px-3 py-2 text-sm rounded-lg border border-slate-200 dark:border-[#484f58] bg-white dark:bg-[#21262d] text-slate-900 dark:text-[#c9d1d9] placeholder-slate-400 dark:placeholder-[#636c76] focus:outline-none focus:ring-2 focus:ring-indigo-500/40 focus:border-indigo-400 transition-colors"
            placeholder={t('report.presetNamePlaceholder')}
            bind:value={editingPresetName}
          />
        </div>
        <div>
          <label for="report-preset-prompt" class="block text-xs font-medium text-slate-500 dark:text-[#7d8590] mb-1.5">{t('report.promptLabel')}</label>
          <textarea
            id="report-preset-prompt"
            class="w-full px-3 py-2 text-sm rounded-lg border border-slate-200 dark:border-[#484f58] bg-white dark:bg-[#21262d] text-slate-900 dark:text-[#c9d1d9] placeholder-slate-400 dark:placeholder-[#636c76] focus:outline-none focus:ring-2 focus:ring-indigo-500/40 focus:border-indigo-400 transition-colors resize-y min-h-[160px] leading-relaxed"
            placeholder={t('report.presetPromptPlaceholder')}
            bind:value={editingPresetPrompt}
            rows="6"
          ></textarea>
        </div>
      </div>
      <div class="modal-footer">
        <button
          class="px-4 py-2 text-sm font-medium rounded-lg text-slate-700 dark:text-[#7d8590] hover:bg-slate-100 dark:hover:bg-[#30363d] transition-colors"
          on:click={() => { showPresetModal = false; }}
        >
          {t('report.cancelEdit')}
        </button>
        <button
          class="px-4 py-2 text-sm font-medium rounded-lg bg-indigo-500 hover:bg-indigo-600 text-white shadow-sm dark:shadow-none transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          disabled={!editingPresetName.trim() || !editingPresetPrompt.trim() || presetSaving}
          on:click={async () => {
            if (presetSaving) return;
            presetSaving = true;
            try {
              const presets = [...(config.daily_report_prompt_presets || [])];
              const entry = { name: editingPresetName.trim(), prompt: editingPresetPrompt.trim() };
              if (editingPresetIndex >= 0) {
                presets[editingPresetIndex] = entry;
              } else {
                presets.push(entry);
              }
              config.daily_report_prompt_presets = presets;
              await savePresets();
              showPresetModal = false;
            } finally {
              presetSaving = false;
            }
          }}
        >
          {#if presetSaving}
            <span class="inline-flex items-center gap-1.5">
              <span class="w-3.5 h-3.5 border-2 border-white border-t-transparent rounded-full animate-spin"></span>
              {t('report.saving')}
            </span>
          {:else}
            {t('report.saveSection')}
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

{#if showBatchExportModal}
  <div class="modal-overlay" role="dialog" aria-modal="true" aria-labelledby="report-batch-export-dialog-title">
    <button type="button" class="absolute inset-0 cursor-default" aria-label={t('report.cancelEdit')} on:click={() => { if (!batchExporting) showBatchExportModal = false; }}></button>
    <div class="modal-panel relative z-10" style="max-width: 32rem;">
      <div class="modal-header">
        <h3 id="report-batch-export-dialog-title" class="modal-title">{t('report.batchExportModalTitle')}</h3>
        <button
          class="modal-close"
          on:click={() => { if (!batchExporting) showBatchExportModal = false; }}
          disabled={batchExporting}
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
        </button>
      </div>
      <div class="modal-body space-y-4">
        <p class="text-xs text-slate-500 dark:text-[#7d8590]">{t('report.batchExportHint')}</p>

        <div class="flex flex-wrap gap-2">
          <button class="page-control-btn" on:click={() => applyBatchPreset('thisWeek')}>{t('report.batchPresetThisWeek')}</button>
          <button class="page-control-btn" on:click={() => applyBatchPreset('lastWeek')}>{t('report.batchPresetLastWeek')}</button>
          <button class="page-control-btn" on:click={() => applyBatchPreset('thisMonth')}>{t('report.batchPresetThisMonth')}</button>
          <button class="page-control-btn" on:click={() => applyBatchPreset('lastMonth')}>{t('report.batchPresetLastMonth')}</button>
        </div>

        <div class="grid gap-3 grid-cols-2">
          <label class="block">
            <span class="text-xs font-medium text-slate-500 dark:text-[#7d8590]">{t('report.batchStartDate')}</span>
            <input
              type="date"
              bind:value={batchStartDate}
              max={getLocalDateString()}
              class="mt-1 w-full px-3 py-2 text-sm rounded-lg border border-slate-200 dark:border-[#484f58] bg-white dark:bg-[#21262d] text-slate-900 dark:text-[#c9d1d9] focus:outline-none focus:ring-2 focus:ring-indigo-500/40"
            />
          </label>
          <label class="block">
            <span class="text-xs font-medium text-slate-500 dark:text-[#7d8590]">{t('report.batchEndDate')}</span>
            <input
              type="date"
              bind:value={batchEndDate}
              max={getLocalDateString()}
              class="mt-1 w-full px-3 py-2 text-sm rounded-lg border border-slate-200 dark:border-[#484f58] bg-white dark:bg-[#21262d] text-slate-900 dark:text-[#c9d1d9] focus:outline-none focus:ring-2 focus:ring-indigo-500/40"
            />
          </label>
        </div>
      </div>
      <div class="modal-footer">
        <button
          class="px-4 py-2 text-sm font-medium rounded-lg text-slate-700 dark:text-[#7d8590] hover:bg-slate-100 dark:hover:bg-[#30363d] transition-colors"
          on:click={() => { if (!batchExporting) showBatchExportModal = false; }}
          disabled={batchExporting}
        >
          {t('report.cancelEdit')}
        </button>
        <button
          class="px-4 py-2 text-sm font-medium rounded-lg bg-indigo-500 hover:bg-indigo-600 text-white shadow-sm dark:shadow-none transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          on:click={exportReportsRange}
          disabled={batchExporting || !batchStartDate || !batchEndDate}
        >
          {#if batchExporting}
            <span class="inline-flex items-center gap-1.5">
              <span class="w-3.5 h-3.5 border-2 border-white border-t-transparent rounded-full animate-spin"></span>
              {t('report.batchExporting')}
            </span>
          {:else}
            {t('report.batchExportConfirm')}
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}
