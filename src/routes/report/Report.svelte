<script>
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { marked } from 'marked';
  import { cache } from '../../lib/stores/cache.js';

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
  let isYesterdayReport = false; // 标记是否显示的是昨日日报
  let config = null; // 当前配置
  let lastLoadedDate = '';

  // 获取 AI 模式显示名称
  function getAiModeName(mode) {
    const modeNames = {
      'local': '基础模板',
      'summary': 'AI 增强',
      'cloud': '云端分析'
    };
    return modeNames[mode] || mode || '未知';
  }

  async function loadConfig() {
    try {
      config = await invoke('get_config');
    } catch (e) {
      console.error('加载配置失败:', e);
    }
  }

  async function loadReport() {
    // 乐观更新：先显示缓存数据
    let cacheData;
    const unsubscribe = cache.subscribe(c => { cacheData = c; });
    unsubscribe();
    
    if (cacheData.reports[selectedDate]?.data) {
      report = cacheData.reports[selectedDate].data;
      isYesterdayReport = false;
      loading = false;
      
      // 缓存有效则直接返回
      if (cache.isValid(cacheData.reports, selectedDate)) {
        return;
      }
      
      // 后台静默刷新
      try {
        const savedReport = await invoke('get_saved_report', { date: selectedDate });
        if (savedReport) {
          report = savedReport;
          cache.setReport(selectedDate, savedReport);
        }
      } catch (e) {
        console.warn('后台刷新日报失败:', e);
      }
    } else {
      // 首次加载
      loading = true;
      error = null;
      try {
        const savedReport = await invoke('get_saved_report', { date: selectedDate });
        if (savedReport) {
          report = savedReport;
          isYesterdayReport = false;
          cache.setReport(selectedDate, savedReport);
        } else {
          // 如果选择今天且今天无日报，尝试加载昨日日报
          if (selectedDate === getLocalDateString()) {
            const yesterday = getYesterdayDateString();
            const yesterdayReport = await invoke('get_saved_report', { date: yesterday });
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
        error = e.toString();
      } finally {
        loading = false;
      }
    }
  }

  function selectDate(date) {
    if (!date || date === selectedDate) return;
    selectedDate = date;
  }

  async function generateReport(force = true) {
    generating = true;
    error = null;
    try {
      // 只有强制生成的时候才会覆盖已有日报（后端默认规则，这里force指定传入）。
      const content = await invoke('generate_report', { date: selectedDate, force });
      report = { date: selectedDate, content, created_at: Date.now() / 1000 };
      isYesterdayReport = false;
      cache.setReport(selectedDate, report);
    } catch (e) {
      error = e.toString();
    } finally {
      generating = false;
    }
  }

  function renderMarkdown(content) {
    return marked(content);
  }

  function formatReportDate(dateStr) {
    const date = new Date(dateStr);
    return date.toLocaleDateString('zh-CN', { year: 'numeric', month: 'long', day: 'numeric', weekday: 'long' });
  }

  $: if (selectedDate && selectedDate !== lastLoadedDate) {
    lastLoadedDate = selectedDate;
    report = null;
    isYesterdayReport = false;
    loadReport();
  }

  onMount(() => {
    loadConfig();
    
    // 如果当前选中的是今天，跨越午夜时自动更新。
    const timer = setInterval(() => {
        const newToday = getLocalDateString();
        // 这里需要保留跨夜逻辑，如果一直停留在当天并跨夜，可能需要更新
        if (selectedDate === newToday) {
            // 目前已经是新的一天，用户正好选了新的一天，不用特殊处理
        }
    }, 60000);
    
    return () => clearInterval(timer);
  });
</script>

<div class="page-shell">
  <!-- 页面标题 -->
  <div class="page-header">
    <div class="page-title-group">
      <div class="page-title-badge">
        <svg fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M8 7h8M8 12h8M8 17h5" />
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M7 3h7l5 5v10a3 3 0 01-3 3H7a3 3 0 01-3-3V6a3 3 0 013-3Z" />
        </svg>
      </div>
      <div class="page-title-copy">
        <h2>
          {selectedDate === getLocalDateString() ? '今日日报' : '历史日报'}
        </h2>
        <p>
        {formatReportDate(selectedDate)}
        {#if config}
          <span class="ml-1.5 {config.ai_mode === 'summary' ? 'page-inline-chip-brand' : 'page-inline-chip-muted'}">
            {getAiModeName(config.ai_mode)}
          </span>
          {#if config.ai_mode === 'summary' && config.text_model?.model}
            <span class="ml-1 page-inline-chip-muted">
              {config.text_model.model}
            </span>
          {/if}
        {/if}
        </p>
      </div>
    </div>
    <div class="flex flex-col items-end gap-2">
      <div class="page-toolbar-end">
        <button
          class="page-control-btn {selectedDate === getLocalDateString() ? 'page-control-btn-active' : ''}"
          on:click={() => selectDate(getLocalDateString())}
        >
          今天
        </button>
        <button
          class="page-control-btn {selectedDate === getYesterdayDateString() ? 'page-control-btn-active' : ''}"
          on:click={() => selectDate(getYesterdayDateString())}
        >
          昨天
        </button>
        <input 
          type="date"
          max={getLocalDateString()}
          bind:value={selectedDate}
          class="page-control-input"
        />
        <span class="page-help-text">可切换到任意历史日期查看历史日报</span>
      </div>
      {#if report}
        <button
          class="page-action-warn"
          on:click={() => generateReport(true)}
          disabled={generating}
        >
          {#if generating}
            <div class="animate-spin rounded-full h-4 w-4 border-2 border-white border-t-transparent"></div>
            生成中...
          {:else}
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
            重新生成
          {/if}
        </button>
      {/if}
    </div>
  </div>

  <!-- 日报内容 -->
  {#if loading}
    <div class="empty-state-lg">
      <div class="animate-spin rounded-full h-8 w-8 border-2 border-indigo-500 border-t-transparent mx-auto"></div>
      <p class="empty-state-copy mt-4">加载中...</p>
    </div>
  {:else if error}
    <div class="page-banner-error">
      <div>
        <div class="flex items-center gap-3 text-red-500 mb-2">
        <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
        </svg>
        <span class="font-medium">生成失败</span>
      </div>
      <p class="text-sm">{error}</p>
      </div>
      <button class="page-action-brand" on:click={() => generateReport(true)}>重试</button>
    </div>
  {:else if report}
    <!-- 昨日日报提示 -->
    {#if isYesterdayReport}
      <div class="page-banner-warning mb-4">
        <div class="flex items-center gap-2 text-sm">
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          今日日报尚未生成，显示的是昨日日报 ({formatReportDate(report.date)})
        </div>
        <button
          class="page-action-warn min-h-9 px-3 text-xs rounded-lg shadow-none"
          on:click={() => generateReport(false)}
          disabled={generating}
        >
          {#if generating}
            <div class="animate-spin rounded-full h-3 w-3 border-2 border-white border-t-transparent"></div>
          {:else}
            ✨ 生成今日日报
          {/if}
        </button>
      </div>
    {/if}
    <div class="page-card">
      <div class="text-xs text-slate-400 mb-4 flex items-center gap-2">
        <div class="w-1.5 h-1.5 rounded-full {isYesterdayReport ? 'bg-amber-500' : 'bg-emerald-500'}"></div>
        {isYesterdayReport ? '昨日日报 - ' : ''}生成于 {new Date(report.created_at * 1000).toLocaleString('zh-CN')}
      </div>
      <div class="markdown-body prose prose-slate dark:prose-invert max-w-none">
        {@html renderMarkdown(report.content)}
      </div>
    </div>
    {:else}
    <div class="empty-state-lg">
      <div class="empty-state-icon !w-16 !h-16 !mb-5 bg-amber-50 dark:bg-amber-950/30">
        <span class="text-3xl">📝</span>
      </div>
      <h3 class="empty-state-title">
        {selectedDate === getLocalDateString() ? '今日暂无日报' : `${selectedDate} 暂无日报`}
      </h3>
      <p class="empty-state-copy mb-5">
        AI 将根据当天的活动记录生成工作总结
      </p>
      <button
        class="page-action-warn min-h-11 px-6 py-3"
        on:click={() => generateReport(false)}
        disabled={generating}
      >
        {#if generating}
          <div class="inline-flex items-center gap-2">
            <div class="animate-spin rounded-full h-4 w-4 border-2 border-white border-t-transparent"></div>
            生成中...
          </div>
        {:else}
          ✨ 生成{selectedDate === getLocalDateString() ? '今日' : '该日'}日报
        {/if}
      </button>
    </div>
  {/if}
</div>

<style>
  .markdown-body :global(h1),
  .markdown-body :global(h2),
  .markdown-body :global(h3) {
    margin-top: 1.5em;
    margin-bottom: 0.5em;
  }
  .markdown-body :global(h1:first-child),
  .markdown-body :global(h2:first-child) {
    margin-top: 0;
  }
  .markdown-body :global(ul),
  .markdown-body :global(ol) {
    padding-left: 1.5em;
  }
  .markdown-body :global(li) {
    margin: 0.25em 0;
  }
  
  /* Excel 风格表格样式 */
  .markdown-body :global(table) {
    width: 100%;
    border-collapse: collapse;
    margin: 1em 0;
    font-size: 0.875rem;
    border: 1px solid #e2e8f0;
    border-radius: 0.5rem;
    overflow: hidden;
  }
  .markdown-body :global(thead) {
    background: linear-gradient(to bottom, #f8fafc, #f1f5f9);
  }
  :global(.dark) .markdown-body :global(thead) {
    background: linear-gradient(to bottom, #334155, #1e293b);
  }
  .markdown-body :global(th) {
    padding: 0.75rem 1rem;
    text-align: center;
    font-weight: 600;
    color: #475569;
    border-bottom: 2px solid #e2e8f0;
    white-space: nowrap;
  }
  :global(.dark) .markdown-body :global(th) {
    color: #e2e8f0;
    border-bottom-color: #475569;
  }
  .markdown-body :global(td) {
    padding: 0.625rem 1rem;
    border-bottom: 1px solid #e2e8f0;
    color: #334155;
    text-align: center;
  }
  :global(.dark) .markdown-body :global(td) {
    border-bottom-color: #334155;
    color: #cbd5e1;
  }
  .markdown-body :global(tbody tr:hover) {
    background-color: #f8fafc;
  }
  :global(.dark) .markdown-body :global(tbody tr:hover) {
    background-color: #1e293b;
  }
  .markdown-body :global(tbody tr:last-child td) {
    border-bottom: none;
  }
  /* 第一列左对齐 */
  .markdown-body :global(td:first-child),
  .markdown-body :global(th:first-child) {
    text-align: left;
    font-weight: 500;
  }
  /* 最后一列右对齐 */
  .markdown-body :global(td:last-child),
  .markdown-body :global(th:last-child) {
    text-align: right;
  }
</style>
