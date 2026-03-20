<script>
  import { createEventDispatcher } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { ask } from '@tauri-apps/plugin-dialog';
  import { cache } from '../../../lib/stores/cache.js';
  import { showToast } from '$lib/stores/toast.js';
  
  export let config;
  export let storageStats = null;
  
  const dispatch = createEventDispatcher();
  let isClearing = false;

  function clearCache() {
    cache.clear();
    showToast('缓存已清理');
    dispatch('clearCache');
  }

  async function clearOldData() {
    const confirmed = await ask('确认删除今天之前的所有活动记录和截图？此操作不可恢复！', {
      title: '确认清理历史数据',
      kind: 'warning',
    });

    if (!confirmed) {
      return;
    }
    
    isClearing = true;
    try {
      const result = await invoke('clear_old_activities');
      showToast(result.message);
      cache.clear();
      dispatch('clearCache');
    } catch (e) {
      showToast('清理失败: ' + e, 'error');
    } finally {
      isClearing = false;
    }
  }

  function handleChange() {
    dispatch('change', config);
  }

  // 计算存储使用百分比
  $: usagePercent = storageStats 
    ? Math.min(Math.round((storageStats.total_size_mb / storageStats.storage_limit_mb) * 100), 100) 
    : 0;

  // 使用量颜色
  $: usageColor = usagePercent > 80 ? 'bg-red-500' : usagePercent > 50 ? 'bg-amber-500' : 'bg-emerald-500';
</script>

<!-- 记录设置 -->
<div class="settings-card mb-5">
  <h3 class="settings-card-title">记录设置</h3>
  <p class="settings-card-desc">控制活动记录的频率和保留策略</p>
  
  <div class="settings-section">
    <!-- 轮询间隔 -->
    <div class="settings-block">
      <div class="flex items-center justify-between">
        <label for="screenshot-interval" class="settings-text">活动轮询间隔</label>
        <span class="settings-value">{config.screenshot_interval}秒</span>
      </div>
      <input
        id="screenshot-interval"
        type="range"
        bind:value={config.screenshot_interval}
        on:change={handleChange}
        min="10"
        max="120"
        step="5"
        class="range-input"
      />
      <div class="flex justify-between text-xs settings-subtle">
        <span>10秒（更精确）</span>
        <span>120秒（更省电）</span>
      </div>
      <p class="settings-note">每隔此时长检测一次当前活动窗口并执行 OCR</p>
    </div>

    <!-- 数据保留 -->
    <div class="settings-block">
      <div class="flex items-center justify-between">
        <label for="retention-days" class="settings-text">数据保留天数</label>
        <span class="settings-value">{config.storage.screenshot_retention_days}天</span>
      </div>
      <input
        id="retention-days"
        type="range"
        bind:value={config.storage.screenshot_retention_days}
        on:change={() => {
          config.storage.metadata_retention_days = config.storage.screenshot_retention_days;
          handleChange();
        }}
        min="1"
        max="90"
        step="1"
        class="range-input"
      />
      <div class="flex justify-between text-xs settings-subtle">
        <span>1天</span>
        <span>90天</span>
      </div>
      <p class="settings-note">超过此天数的活动记录和截图将被自动清理</p>
    </div>
  </div>
</div>

<!-- 存储统计 -->
{#if storageStats}
<div class="settings-card mb-5">
  <h3 class="settings-card-title !mb-4">存储使用</h3>
  
  <!-- 存储进度条 -->
  <div class="mb-5">
    <div class="flex items-end justify-between mb-2">
      <div>
        <span class="text-2xl font-bold text-slate-800 dark:text-white">{storageStats.total_size_mb}</span>
        <span class="settings-muted"> / {storageStats.storage_limit_mb} MB</span>
      </div>
      <span class="text-sm font-medium {usagePercent > 80 ? 'settings-text-danger' : 'settings-muted'}">{usagePercent}%</span>
    </div>
    <div class="w-full h-2.5 bg-slate-100 dark:bg-slate-700 rounded-full overflow-hidden">
      <div 
        class="h-full rounded-full transition-all duration-500 {usageColor}"
        style="width: {usagePercent}%"
      ></div>
    </div>
  </div>

  <!-- 统计卡片 -->
  <div class="grid grid-cols-3 gap-3">
    <div class="text-center p-3 bg-slate-50 dark:bg-slate-700/30 rounded-xl">
      <p class="text-xl font-bold text-slate-800 dark:text-white">{storageStats.total_files}</p>
      <p class="settings-muted mt-0.5">截图数</p>
    </div>
    <div class="text-center p-3 bg-slate-50 dark:bg-slate-700/30 rounded-xl">
      <p class="text-xl font-bold text-slate-800 dark:text-white">{storageStats.total_size_mb} MB</p>
      <p class="settings-muted mt-0.5">已用空间</p>
    </div>
    <div class="text-center p-3 bg-slate-50 dark:bg-slate-700/30 rounded-xl">
      <p class="text-xl font-bold text-slate-800 dark:text-white">{storageStats.retention_days} 天</p>
      <p class="settings-muted mt-0.5">保留期限</p>
    </div>
  </div>
</div>

<!-- 数据管理 -->
<div class="settings-card">
  <h3 class="settings-card-title !mb-4">数据管理</h3>
  <div class="settings-block">
    <!-- 清理缓存 -->
    <div class="flex items-center justify-between p-3 bg-slate-50 dark:bg-slate-700/30 rounded-xl">
      <div>
        <p class="settings-text">清理页面缓存</p>
        <p class="settings-muted mt-0.5">解决数据显示异常问题，不影响已保存的数据</p>
      </div>
      <button
        on:click={clearCache}
        class="settings-action-secondary"
      >
        清理缓存
      </button>
    </div>
    
    <!-- 清理历史 -->
    <div class="settings-panel-danger flex items-center justify-between">
      <div>
        <p class="settings-text-danger text-sm font-medium">清理历史数据</p>
        <p class="settings-muted mt-0.5">删除今天之前的所有活动记录和截图，不可恢复</p>
      </div>
      <button
        on:click={clearOldData}
        disabled={isClearing}
        class="settings-action-danger"
      >
        {#if isClearing}
          清理中...
        {:else}
          清理历史
        {/if}
      </button>
    </div>
  </div>
</div>
{/if}
