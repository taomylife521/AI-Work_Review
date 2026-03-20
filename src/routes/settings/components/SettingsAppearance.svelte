<script>
  import { createEventDispatcher, onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { showToast } from '$lib/stores/toast.js';

  export let config;

  const dispatch = createEventDispatcher();

  // === 背景图片 ===
  let bgPreview = null;
  let bgUploading = false;

  const blurLabels = ['清晰', '轻微模糊', '中等模糊'];

  onMount(async () => {
    try {
      const b64 = await invoke('get_background_image');
      if (b64) bgPreview = `data:image/jpeg;base64,${b64}`;
    } catch (e) { /* ignore */ }
  });

  function handleBgFileSelect(event) {
    const file = event.target.files?.[0];
    if (!file) return;
    if (!file.type.startsWith('image/')) return;
    if (file.size > 10 * 1024 * 1024) {
      showToast('图片大小不能超过 10MB', 'warning');
      return;
    }

    bgUploading = true;
    const reader = new FileReader();
    reader.onload = async () => {
      try {
        const b64Data = reader.result.split(',')[1];
        await invoke('save_background_image', { data: b64Data });
        config.background_image = 'background.jpg';
        const freshB64 = await invoke('get_background_image');
        const imageUrl = freshB64 ? `data:image/jpeg;base64,${freshB64}` : null;
        bgPreview = imageUrl;
        dispatchBgEvent(imageUrl);
      } catch (e) {
        console.error('上传背景图失败:', e);
        showToast('上传失败: ' + e, 'error');
      } finally {
        bgUploading = false;
      }
    };
    reader.readAsDataURL(file);
  }

  async function clearBg() {
    try {
      await invoke('clear_background_image');
      bgPreview = null;
      config.background_image = null;
      dispatchBgEvent(null);
    } catch (e) {
      console.error('清除背景图失败:', e);
      showToast('清除背景图失败: ' + e, 'error');
    }
  }

  function updateBgOpacity(val) {
    config.background_opacity = parseFloat(val);
    dispatch('change', config);
    dispatchBgEvent(bgPreview);
  }

  function updateBgBlur(val) {
    config.background_blur = parseInt(val);
    dispatch('change', config);
    dispatchBgEvent(bgPreview);
  }

  function dispatchBgEvent(image) {
    window.dispatchEvent(new CustomEvent('background-changed', {
      detail: {
        image,
        opacity: config.background_opacity ?? 0.25,
        blur: config.background_blur ?? 1,
      }
    }));
  }
</script>

<!-- 背景图片 -->
<div class="settings-card">
  <h3 class="settings-card-title">背景图片</h3>
  <p class="settings-card-desc">上传图片作为应用背景底纹</p>

  <div class="settings-section">
    <!-- 预览 + 上传 -->
    <div class="flex items-start gap-4">
      {#if bgPreview}
        <div class="w-32 h-20 rounded-lg overflow-hidden border border-slate-200 dark:border-slate-700 flex-shrink-0">
          <img src={bgPreview} alt="背景预览" class="w-full h-full object-cover" />
        </div>
      {:else}
        <div class="w-32 h-20 rounded-lg border-2 border-dashed border-slate-200 dark:border-slate-700 flex items-center justify-center flex-shrink-0">
          <span class="settings-subtle">无背景</span>
        </div>
      {/if}

      <div class="flex-1 settings-field">
        <label class="settings-action-secondary cursor-pointer">
          {#if bgUploading}
            <div class="animate-spin rounded-full h-3 w-3 border-2 border-slate-500 border-t-transparent"></div>
            处理中...
          {:else}
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" /></svg>
            选择图片
          {/if}
          <input type="file" accept="image/*" class="hidden" on:change={handleBgFileSelect} disabled={bgUploading} />
        </label>
        {#if bgPreview}
          <button
            on:click={clearBg}
            class="settings-link-danger"
          >
            清除背景
          </button>
        {/if}
        <p class="settings-muted">支持 JPG/PNG，建议不超过 10MB</p>
      </div>
    </div>

    {#if bgPreview || config.background_image}
      <hr class="border-slate-200 dark:border-slate-700" />

      <!-- 显示强度 -->
      <div class="settings-block">
        <div class="flex items-center justify-between">
          <span class="settings-text">显示强度</span>
          <span class="settings-value">{Math.round((config.background_opacity ?? 0.25) * 100)}%</span>
        </div>
        <input
          type="range"
          min="0.05"
          max="0.60"
          step="0.01"
          value={config.background_opacity ?? 0.25}
          on:input={(e) => updateBgOpacity(e.target.value)}
          class="range-input"
        />
        <div class="flex justify-between text-[10px] settings-subtle">
          <span>淡雅</span>
          <span>浓郁</span>
        </div>
      </div>

      <!-- 模糊度 -->
      <div class="settings-block">
        <div class="flex items-center justify-between">
          <span class="settings-text">模糊程度</span>
          <span class="settings-muted">{blurLabels[config.background_blur ?? 1]}</span>
        </div>
        <div class="flex gap-2">
          {#each [0, 1, 2] as level}
            <button
              on:click={() => updateBgBlur(level)}
              class="segment-btn
                {(config.background_blur ?? 1) === level
                  ? 'settings-segment-active'
                  : 'settings-segment-base'}"
            >
              {blurLabels[level]}
            </button>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</div>
