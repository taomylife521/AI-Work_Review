<script>
  import { afterUpdate, onDestroy, onMount, tick } from 'svelte';
  import { fly } from 'svelte/transition';
  import { invoke, Channel } from '@tauri-apps/api/core';
  import { marked } from 'marked';
  import { assistantStore, BASIC_ASSISTANT_MODEL_ID } from '../../lib/stores/assistant.js';
  import { buildHistoryPayload } from './historyPayload.js';
  import { createRequestEventGate } from './requestEventGate.js';
  import { reduceStreamEvent } from './streamEvent.js';
  import { formatDurationLocalized, locale, t, tm, translateCategoryLabel } from '$lib/i18n/index.js';

  marked.use({
    gfm: true,
    breaks: true,
  });

  let input = '';
  let error = null;
  let chatBody;
  let composer;
  let bottomAnchor;
  let assistantState = {};
  let unsubscribeAssistant = () => {};
  let destroyed = false;
  let activeSendingRequestId = null;
  let stickToBottom = true;
  $: sending = assistantState.sending ?? false;
  $: messages = assistantState.messages ?? [];
  $: currentLocale = $locale;
  $: starterPrompts = dynamicPrompts.length ? dynamicPrompts : (tm('ask.starterPrompts') || []);
  let dynamicPrompts = [];

  // 模型选择器
  let modelProfiles = [];
  let selectedModelId = BASIC_ASSISTANT_MODEL_ID;
  let modelSelectEl;
  let modelSelectWidth = 'auto';
  let modelMeasureEl;

  const providerDisplayNames = {
    ollama: {
      'zh-CN': 'Ollama (本地)',
      en: 'Ollama (Local)',
      'zh-TW': 'Ollama（本機）',
    },
    openai: {
      'zh-CN': 'OpenAI 兼容',
      en: 'OpenAI Compatible',
      'zh-TW': 'OpenAI 相容',
    },
    siliconflow: {
      'zh-CN': '硅基流动',
      en: 'SiliconFlow',
      'zh-TW': '矽基流動',
    },
    deepseek: {
      'zh-CN': 'DeepSeek',
      en: 'DeepSeek',
      'zh-TW': 'DeepSeek',
    },
    qwen: {
      'zh-CN': '通义千问',
      en: 'Qwen',
      'zh-TW': '通義千問',
    },
    zhipu: {
      'zh-CN': '智谱清言',
      en: 'Zhipu',
      'zh-TW': '智譜清言',
    },
    moonshot: {
      'zh-CN': 'Kimi',
      en: 'Moonshot Kimi',
      'zh-TW': 'Kimi',
    },
    doubao: {
      'zh-CN': '豆包',
      en: 'Doubao',
      'zh-TW': '豆包',
    },
    minimax: {
      'zh-CN': 'MiniMax',
      en: 'MiniMax',
      'zh-TW': 'MiniMax',
    },
    gemini: {
      'zh-CN': 'Google Gemini',
      en: 'Google Gemini',
      'zh-TW': 'Google Gemini',
    },
    claude: {
      'zh-CN': 'Anthropic Claude',
      en: 'Anthropic Claude',
      'zh-TW': 'Anthropic Claude',
    },
  };

  function localizedProviderName(providerId) {
    return providerDisplayNames[providerId]?.[currentLocale] || providerId || '';
  }

  function displayModelProfileName(profile) {
    if (!profile) return '';
    // 优先用 profile.name（后端 default_profile_name 已拼好完整显示名，或用户自定义名）。
    // 避免再次拼接 provider · model_id，那样会与后端重复且暴露裸 API id（如 Qwen/Qwen3-8B）。
    const profileName = profile.name?.trim();
    if (profileName) {
      return profileName;
    }
    // fallback：profile.name 缺失时才用 provider · model 拼一个
    const localizedProvider = localizedProviderName(profile.model_config?.provider);
    const modelName = profile.model_config?.model?.trim();
    if (localizedProvider && modelName) {
      return `${localizedProvider} · ${modelName}`;
    }
    if (modelName) {
      return modelName;
    }
    return '';
  }

  // 当前选中项的显示文本（用于测量 select 收起态宽度）
  function currentModelOptionLabel() {
    if (selectedModelId === BASIC_ASSISTANT_MODEL_ID) {
      return t('ask.basicTemplate');
    }
    const profile = modelProfiles.find((p) => p.id === selectedModelId);
    return profile ? displayModelProfileName(profile) || t('ask.aiEnhanced') : '';
  }

  // Measure collapsed select width: text width + padding(px-3=24) + arrow(pr-8=32) + border(2) ≈ text + 46.
  // Use a hidden mirror span with the select's font props to measure precisely,
  // avoiding width:max-content which sizes to the longest option.
  function measureModelSelectWidth() {
    if (!modelMeasureEl) return;
    modelMeasureEl.textContent = currentModelOptionLabel() || '';
    const textWidth = modelMeasureEl.offsetWidth;
    const clamped = Math.max(textWidth + 46, 72); // min 72px, max aligns with max-w-[260px]
    modelSelectWidth = Math.min(clamped, 260) + 'px';
  }

  // Re-measure when selection / profile list / locale changes
  $: measureModelSelectWidth(selectedModelId, modelProfiles, currentLocale);

  onMount(async () => {
    unsubscribeAssistant = assistantStore.subscribe((state) => {
      assistantState = state;
      const nextMessages = state.messages || [];
      const previousCount = messages.length;
      const messageCountIncreased = nextMessages.length > previousCount;
      const latestMessage = nextMessages[nextMessages.length - 1];

      selectedModelId = state.selectedModelId || BASIC_ASSISTANT_MODEL_ID;

      if (!nextMessages.length) {
        stickToBottom = true;
        return;
      }

      if (previousCount === 0) {
        void scrollToBottom('auto', 3);
        return;
      }

      if (messageCountIncreased && (stickToBottom || latestMessage?.role === 'user')) {
        void scrollToBottom(latestMessage?.role === 'assistant' ? 'smooth' : 'auto', 2);
      }
    });

    // 加载模型档案
    try {
      const config = await invoke('get_config');
      modelProfiles = config.text_model_profiles || [];
      if (
        selectedModelId !== BASIC_ASSISTANT_MODEL_ID &&
        !modelProfiles.some((profile) => profile.id === selectedModelId)
      ) {
        selectedModelId = BASIC_ASSISTANT_MODEL_ID;
        assistantStore.setSelectedModelId(BASIC_ASSISTANT_MODEL_ID);
      }
    } catch (e) {
      console.warn('加载模型配置失败:', e);
    }

    resizeComposer();
    await scrollToBottom('auto', 3);
    composer?.focus();

    // 配了 AI 模型时，基于当前工作记录动态生成 starter prompts（替代固定 4 条）
    refreshDynamicPrompts();
  });

  onDestroy(() => {
    destroyed = true;
    if (activeSendingRequestId) {
      assistantStore.finishSending(activeSendingRequestId);
      activeSendingRequestId = null;
    }
    unsubscribeAssistant();
  });

  function sourceLabel(sourceType) {
    const labels = {
      activity: t('ask.referenceTypes.activity'),
      hourly_summary: t('ask.referenceTypes.hourly_summary'),
      daily_report: t('ask.referenceTypes.daily_report'),
    };
    return labels[sourceType] || sourceType;
  }

  // 已知的段落标题——后端模板和 AI 模型都可能输出这些词作为独立行
  const SECTION_TITLES = new Set([
    '结论', '依据', '关键发现', '本期概览', '重点工作',
    '核心观察', '风险与提醒', '下阶段建议', '工作复盘',
    '主要意图', '主要工作', '待跟进事项', '代表性 Session',
    '相关记录依据',
  ]);
  const renderedMarkdownCache = new Map();
  // Streaming render throttle: reuse last HTML within STREAM_RENDER_INTERVAL_MS,
  // so we don't run marked.parse on every token.
  const STREAM_RENDER_INTERVAL_MS = 250;
  const streamRenderState = new Map(); // messageIndex -> { html, at }

  function normalizeAssistantContent(content) {
    const text = (content || '').replace(/\r\n/g, '\n').trim();
    if (!text) return '';

    const lines = text.split('\n');

    // ——— 第 1 步：去掉模板自引用句 ———
    const filtered = [];
    let inCodeBlock = false;
    for (const line of lines) {
      const t = line.trim();
      if (t.startsWith('```')) inCodeBlock = !inCodeBlock;
      if (!inCodeBlock && (
        t.includes('我基于周报复盘') ||
        t.includes('我基于意图识别') ||
        t.includes('我基于 Session 聚合') ||
        t.includes('我基于记忆检索')
      )) continue;
      filtered.push(line);
    }

    // ——— 第 2 步：逐行补全 markdown 格式（兼容已有部分格式的内容）———
    const result = [];
    inCodeBlock = false;

    for (let i = 0; i < filtered.length; i++) {
      const raw = filtered[i];
      const t = raw.trim();

      // 空行保留（段落分隔）
      if (!t) { result.push(''); continue; }

      // 代码块原样透传
      if (t.startsWith('```')) { inCodeBlock = !inCodeBlock; result.push(raw); continue; }
      if (inCodeBlock) { result.push(raw); continue; }

      // 表格行原样透传（避免被下面的"标题/列表"规则误伤，破坏表格语法）
      if (/^\|.*\|$/.test(t)) {
        result.push(raw);
        continue;
      }

      // 已有 markdown 标题 → 保留
      if (/^#{1,6}\s/.test(t)) {
        result.push(raw);
        continue;
      }

      // 已有列表/引用标记 → 保留
      if (/^[-*+]\s/.test(t) || /^\d+\.\s/.test(t) || /^>\s/.test(t)) {
        result.push(raw);
        continue;
      }

      // 已知段落标题（无 # 前缀的纯文本）→ ## 标题
      if (SECTION_TITLES.has(t)) {
        result.push('', `## ${t}`, '');
        continue;
      }

      // "标题（说明）" 格式 → ### 副标题（排除含"："的行，避免与下方 key:value 规则重叠误判）
      if (/^[^（）()。，！？：]{2,20}[（(].+[）)]$/.test(t) && !t.includes('。')) {
        result.push('', `### ${t}`, '');
        continue;
      }

      // 短 key：value 数据行（key ≤ 6 字符，无句号结尾，总长 < 32，不含反引号）→ 列表项
      // 收窄阈值避免把自然语言句子（如"结论：本次工作正常"）误转成列表项
      if (
        /^[^：。！？，`]{1,6}：/.test(t) &&
        !/[。！？]$/.test(t) &&
        !t.includes('`') &&
        t.length < 32
      ) {
        result.push(`- ${t}`);
        continue;
      }

      // 普通文本
      result.push(t);
    }

    return result.join('\n');
  }

  function renderMarkdown(content) {
    const normalized = normalizeAssistantContent(content);
    if (!normalized) return '';

    const cached = renderedMarkdownCache.get(normalized);
    if (cached) return cached;

    const html = marked.parse(normalized);
    renderedMarkdownCache.set(normalized, html);

    // 控制缓存上限，避免长会话内存持续增长
    if (renderedMarkdownCache.size > 120) {
      const oldestKey = renderedMarkdownCache.keys().next().value;
      renderedMarkdownCache.delete(oldestKey);
    }

    return html;
  }

  // 流式渲染：节流，STREAM_RENDER_INTERVAL_MS 内复用上次 HTML，避免每个 token 都跑 marked.parse。
  // key 用消息在数组中的下标，收尾时由 renderMarkdown 接管（命中缓存，无额外开销）。
  function renderStreamingMarkdown(content, key) {
    const now = Date.now();
    const state = streamRenderState.get(key);
    if (state && now - state.at < STREAM_RENDER_INTERVAL_MS) {
      return state.html;
    }
    const html = renderMarkdown(content);
    streamRenderState.set(key, { html, at: now });
    return html;
  }

  function resizeComposer() {
    if (!composer) return;
    composer.style.height = '0px';
    composer.style.height = `${Math.min(composer.scrollHeight, 220)}px`;
  }

  function isNearBottom(threshold = 120) {
    if (!chatBody) return true;
    return chatBody.scrollHeight - chatBody.scrollTop - chatBody.clientHeight <= threshold;
  }

  function syncStickToBottom() {
    stickToBottom = isNearBottom();
  }

  async function scrollToBottom(behavior = 'smooth', attempts = 1) {
    await tick();
    for (let attempt = 0; attempt < attempts; attempt += 1) {
      await new Promise((resolve) => requestAnimationFrame(resolve));
      if (bottomAnchor?.scrollIntoView) {
        bottomAnchor.scrollIntoView({ block: 'end', behavior });
      } else if (chatBody) {
        chatBody.scrollTop = chatBody.scrollHeight;
      }
    }
  }

  // 流式更新时自动滚动：仅在用户位于底部附近时才滚（主流 chat 体验）
  function autoScrollOnStream() {
    if (destroyed || !stickToBottom) return;
    void scrollToBottom('auto', 1);
  }

  function getSelectedModelConfig() {
    if (selectedModelId === BASIC_ASSISTANT_MODEL_ID) {
      return null;
    }
    const profile = modelProfiles.find((p) => p.id === selectedModelId);
    return profile ? profile.model_config : null;
  }

  function handleModelChange(event) {
    selectedModelId = event.currentTarget.value;
    assistantStore.setSelectedModelId(selectedModelId);
    refreshDynamicPrompts();
  }

  async function clearConversation() {
    assistantStore.clearMessages();
    error = null;
    await tick();
    await scrollToBottom('auto', 2);
    composer?.focus();
  }

  const ASK_TIMEOUT_MS = 120_000;

  function withTimeout(promise, ms) {
    let timer;
    const timeout = new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error(t('ask.timeoutError'))), ms);
    });
    return Promise.race([promise, timeout]).finally(() => clearTimeout(timer));
  }

  async function submitQuestion(question = input) {
    const trimmed = question.trim();
    if (!trimmed || sending) return;

    // 用户主动发送 → 强制切回底部跟随模式
    stickToBottom = true;
    error = null;

    const history = buildHistoryPayload(messages);

    assistantStore.appendMessage({
      role: 'user',
      content: trimmed,
    });

    // 为本次回答绑定稳定 ID；所有流式事件只允许更新这条消息。
    const assistantMessageId =
      typeof crypto !== 'undefined' && crypto.randomUUID
        ? crypto.randomUUID()
        : `ask-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    activeSendingRequestId = assistantMessageId;
    assistantStore.beginSending(assistantMessageId);

    // 发送即插入占位 assistant message，流式事件会逐步更新它（步骤/引用/答案）
    assistantStore.appendMessage({
      id: assistantMessageId,
      role: 'assistant',
      content: '',
      streaming: true,
      steps: [],
      references: [],
      toolLabels: [],
      usedAi: false,
      failed: false,
    });

    input = '';
    resizeComposer();
    await tick();
    await scrollToBottom('auto', 2);

    let streamSettled = false;
    let requestGate = null;
    try {
      const channel = new Channel();
      requestGate = createRequestEventGate({
        isDestroyed: () => destroyed,
        onEvent: (event) => handleStreamEvent(assistantMessageId, event),
      });
      channel.onmessage = (event) => {
        if (requestGate.handle(event)) streamSettled = true;
      };
      const answer = await withTimeout(
        invoke('chat_work_assistant', {
          question: trimmed,
          history,
          modelConfig: getSelectedModelConfig(),
          locale: currentLocale,
          onEvent: channel,
        }),
        ASK_TIMEOUT_MS
      );

      // 事件优先：已收到 done/error 则保留事件内容；否则用 await 返回值兜底。
      assistantStore.updateMessageById(assistantMessageId, (m) => ({
        ...m,
        ...(streamSettled
          ? {}
          : {
              content: answer?.answer?.trim() || t('ask.emptyResponse'),
              references: answer?.references || m.references,
              toolLabels: answer?.toolLabels || m.toolLabels,
              streaming: false,
              failed: false,
            }),
        // Done 事件不携带模型元数据，因此无论是否已收尾都按 ID 补写。
        usedAi: answer?.usedAi ?? m.usedAi,
        modelName: answer?.modelName ?? m.modelName,
      }));
    } catch (e) {
      requestGate?.close();
      if (!destroyed) {
        error = e.toString();
      }
      // 只把错误写入本次占位消息，迟到的旧事件不会影响后续请求。
      assistantStore.updateMessageById(assistantMessageId, (m) => ({
        ...m,
        content: m.content || `${t('ask.requestFailed')}: ${e}`,
        streaming: false,
        failed: true,
      }));
    } finally {
      requestGate?.close();
      assistantStore.finishSending(assistantMessageId);
      if (activeSendingRequestId === assistantMessageId) {
        activeSendingRequestId = null;
      }
      if (destroyed) return;
      await tick();
      resizeComposer();
      composer?.focus();
    }
  }

  // 处理后端流式事件，返回 true 表示终态（done/error）。
  function handleStreamEvent(messageId, event) {
    let terminal = false;
    assistantStore.updateMessageById(messageId, (message) => {
      const result = reduceStreamEvent(message, event, t('ask.requestFailed'));
      terminal = result.terminal;
      return result.message;
    });

    if (event?.type === 'stepStart' || event?.type === 'stepResult' || event?.type === 'token') {
      autoScrollOnStream();
    } else if (event?.type === 'done' && !destroyed) {
      // done：用户在底部时强制滚一次（确保完整内容可见）
      void scrollToBottom('auto', 2);
    }
    return terminal;
  }

  function handleComposerKeydown(event) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      submitQuestion();
    }
  }

  async function refreshDynamicPrompts() {
    // 没配 AI 模型时用固定 starter（i18n），配了才动态生成
    if (selectedModelId === BASIC_ASSISTANT_MODEL_ID) {
      dynamicPrompts = [];
      return;
    }
    const profile = modelProfiles.find((p) => p.id === selectedModelId);
    if (!profile) {
      dynamicPrompts = [];
      return;
    }
    try {
      const stats = await invoke('get_today_stats');
      const recentApps = (stats?.app_usage || []).slice(0, 3).map((a) => a.app_name).join(t('common.listSeparator'));
      const topCategory = translateCategoryLabel(stats?.category_usage?.[0]?.category || '');
      const workMinutes = Math.round((stats?.total_work_duration || 0) / 60);

      const systemPrompt = t('ask.starterSystemPrompt');
      const userPrompt = t('ask.starterUserPrompt', {
        workMinutes,
        recentApps: recentApps || t('common.none'),
        topCategory: topCategory || t('common.none'),
      });

      const result = await invoke('generate_text_with_model', {
        modelConfig: profile.model_config,
        systemPrompt,
        prompt: userPrompt,
      });

      const parsed = JSON.parse(result);
      if (Array.isArray(parsed) && parsed.length > 0) {
        dynamicPrompts = parsed.filter((p) => typeof p === 'string' && p.trim()).slice(0, 4);
      }
    } catch (e) {
      console.warn('动态 starter 生成失败，用固定:', e);
      dynamicPrompts = [];
    }
  }

  $: hasConversation = messages.length > 0;
  $: input, resizeComposer();

  // afterUpdate：每次 DOM 更新后，如果用户在底部附近，直接同步滚到底
  // 这是 Svelte 推荐的"保持滚到底部"方案，比 async scrollToBottom 可靠
  afterUpdate(() => {
    if (stickToBottom && chatBody) {
      chatBody.scrollTop = chatBody.scrollHeight;
    }
  });
</script>

<div class="page-shell ask-workbench-shell h-full" data-locale={currentLocale}>
  <div class="ask-workbench-frame flex h-[calc(100vh-7rem)] flex-col overflow-hidden">
    <div bind:this={chatBody} class="flex-1 overflow-y-auto px-4 pb-40 pt-10" on:scroll={syncStickToBottom}>
      {#if !hasConversation}
        <div class="ask-welcome-panel mx-auto flex min-h-full max-w-4xl flex-col items-center justify-center text-center">
          <span class="ask-kicker">{t('ask.title')}</span>
          <h1 class="mb-2 text-2xl font-semibold tracking-tight text-slate-900 dark:text-[#e6edf3]">{t('ask.title')}</h1>
          <p class="mb-10 text-sm text-slate-500 dark:text-[#7d8590]">{t('ask.subtitle')}</p>
          <div class="ask-starter-grid grid w-full max-w-3xl gap-3 sm:grid-cols-2">
            {#each starterPrompts as prompt}
              <button
                class="ask-starter-card rounded-[28px] bg-[linear-gradient(180deg,rgba(255,255,255,0.92),rgba(248,250,252,0.88))] px-5 py-4 text-left text-sm font-medium leading-6 text-slate-700 ring-1 ring-inset ring-slate-200/80 shadow-[0_8px_24px_rgba(15,23,42,0.04)] transition hover:-translate-y-0.5 hover:text-slate-900 hover:ring-indigo-300/80 hover:shadow-[0_12px_28px_rgba(79,70,229,0.10)] active:scale-[0.98] dark:bg-[linear-gradient(180deg,rgba(15,23,42,0.78),rgba(15,23,42,0.62))] dark:text-[#c9d1d9] dark:ring-[#30363d]/80 dark:shadow-none dark:hover:text-[#e6edf3] dark:hover:ring-indigo-500/60"
                on:click={() => submitQuestion(prompt)}
                disabled={sending}
              >
                {prompt}
              </button>
            {/each}
          </div>
        </div>
      {:else}
        <div class="ask-thread-shell mx-auto flex min-h-full max-w-4xl flex-col gap-10">
          {#each messages as message, messageIndex}
            <div class={message.role === 'user' ? 'flex w-full min-w-0 justify-end' : 'flex w-full min-w-0 justify-start'}>
              <div
                in:fly={{ y: 10, duration: 240 }}
                class={message.role === 'user'
                  ? 'ask-message-card ask-message-card-user min-w-0 max-w-[78%] rounded-[28px] rounded-br-lg bg-gradient-to-br from-indigo-50 to-slate-50 px-5 py-4 text-slate-900 ring-1 ring-inset ring-indigo-200/70 shadow-sm dark:shadow-none dark:from-indigo-950/60 dark:to-[#161b22] dark:text-[#e6edf3] dark:ring-indigo-800/50'
                  : 'ask-message-card ask-message-card-assistant min-w-0 w-full max-w-[90%] text-slate-900 dark:text-[#e6edf3]'}
              >
                {#if message.role === 'assistant'}
                  {#if message.steps?.length}
                    <div class="mb-3 flex flex-col gap-1">
                      {#each message.steps as step, si}
                        <details class="group/step rounded-lg bg-slate-50/60 dark:bg-[#161b22]/30 overflow-hidden">
                          <summary
                            class="flex items-center gap-2 px-2.5 py-1.5 text-xs text-slate-500 dark:text-[#7d8590] cursor-pointer select-none list-none transition-colors hover:bg-slate-100/60 dark:hover:bg-[#21262d]/40"
                            in:fly={{ x: -4, duration: 160 }}
                          >
                            {#if step.status === 'running'}
                              <span class="inline-block h-3 w-3 animate-spin rounded-full border-2 border-indigo-200 border-t-indigo-500 dark:border-indigo-900/60 dark:border-t-indigo-400"></span>
                            {:else if step.ok === false}
                              <span class="inline-block h-1.5 w-1.5 rounded-full bg-rose-500"></span>
                            {:else if step.ok === true}
                              <span class="inline-block h-1.5 w-1.5 rounded-full bg-emerald-400"></span>
                            {:else}
                              <span class="inline-block h-1.5 w-1.5 rounded-full bg-slate-400"></span>
                            {/if}
                            <span class="font-medium">{step.label}</span>
                            {#if step.status === 'done' && step.ok === false}
                              <span class="text-rose-500 dark:text-rose-400">· {t('ask.stepFailed')}</span>
                            {:else if step.status === 'done' && step.tool === 'search_memory' && step.ok === true && step.hits != null}
                              <span class="text-slate-400 dark:text-[#636c76]">· {step.hits} {t('ask.hits')}</span>
                            {/if}
                            {#if step.references?.length}
                              <svg class="w-3 h-3 ml-auto shrink-0 text-slate-400 transition-transform group-open/step:rotate-90" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" /></svg>
                            {/if}
                          </summary>
                          {#if step.references?.length}
                            <div class="px-2.5 pb-2 pt-1 space-y-1 border-t border-slate-200/60 dark:border-[#30363d]/40">
                              {#each step.references as ref}
                                <div class="text-[11px] leading-relaxed text-slate-500 dark:text-[#7d8590]">
                                  {#if ref.app_name}<span class="font-medium text-slate-600 dark:text-[#adbac7]">{ref.app_name}</span> · {/if}
                                  <span>{ref.title}</span>
                                  <span class="text-slate-400 dark:text-[#636c76]">— {ref.date}</span>
                                </div>
                              {/each}
                            </div>
                          {/if}
                        </details>
                      {/each}
                    </div>
                  {/if}
                  <div class="markdown-body assistant-markdown min-w-0 max-w-none">
                    {#if message.streaming}
                      <div class="streaming-content">{#if message.content}{@html renderStreamingMarkdown(message.content, messageIndex)}{:else}<p class="text-slate-400 dark:text-[#7d8590]">{t('ask.thinking')}</p>{/if}<span class="ml-0.5 inline-block animate-pulse text-slate-400 dark:text-[#7d8590] align-text-bottom">▍</span></div>
                    {:else}
                      {@html renderMarkdown(message.content)}
                    {/if}
                  </div>

                  {#if message.references?.length}
                    <details class="mt-6 rounded-[24px] bg-slate-50/74 px-4 py-3 ring-1 ring-inset ring-slate-200/60 dark:bg-[#0d1117]/34 dark:ring-[#21262d]/70">
                      <summary class="cursor-pointer list-none text-sm font-medium text-slate-500 dark:text-[#7d8590]">
                        {t('ask.references', { count: message.references.length })}
                      </summary>

                      <div class="mt-3 space-y-2">
                        {#each message.references as item}
                          <div class="ask-reference-card rounded-[20px] bg-white/88 px-3 py-3 ring-1 ring-inset ring-slate-200/70 dark:bg-[#161b22]/80 dark:ring-[#21262d]">
                            <div class="flex flex-wrap items-center gap-2 text-xs text-slate-400 dark:text-[#636c76]">
                              <span>{sourceLabel(item.sourceType)}</span>
                              <span>{item.date}</span>
                              {#if item.appName}
                                <span>{item.appName}</span>
                              {/if}
                              {#if item.duration}
                                <span>{formatDurationLocalized(item.duration)}</span>
                              {/if}
                            </div>
                            <div class="mt-1 text-sm font-medium text-slate-900 dark:text-[#e6edf3]">{item.title}</div>
                            {#if item.excerpt}
                              <div class="mt-1 text-sm leading-6 text-slate-500 dark:text-[#7d8590]">{item.excerpt}</div>
                            {/if}
                          </div>
                        {/each}
                      </div>
                    </details>
                  {/if}
                {:else}
                  <p class="whitespace-pre-wrap break-words text-[16px] font-medium leading-7 tracking-[0.01em]">{message.content}</p>
                {/if}
              </div>
            </div>
          {/each}

          <!-- Loading bubble -->
          {#if sending}
            <div class="flex justify-start">
              <div class="rounded-[24px] bg-slate-50/80 px-5 py-4 ring-1 ring-inset ring-slate-200/50 dark:bg-[#21262d]/60 dark:ring-[#30363d]/50">
                <div class="flex items-center gap-1.5">
                  <span class="inline-block h-2 w-2 animate-pulse rounded-full bg-slate-400 dark:bg-[#636c76]"></span>
                  <span class="inline-block h-2 w-2 animate-pulse rounded-full bg-slate-400 dark:bg-[#636c76]" style="animation-delay: 0.2s"></span>
                  <span class="inline-block h-2 w-2 animate-pulse rounded-full bg-slate-400 dark:bg-[#636c76]" style="animation-delay: 0.4s"></span>
                </div>
              </div>
            </div>
          {/if}

          <!-- Error callout -->
          {#if error}
            <div
              class="flex items-start gap-3 rounded-[24px] border border-rose-200 bg-rose-50/80 px-5 py-4 dark:border-rose-900/60 dark:bg-rose-950/30"
              in:fly={{ y: -8, duration: 220 }}
            >
              <svg class="mt-0.5 h-5 w-5 shrink-0 text-rose-500 dark:text-rose-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z" />
              </svg>
              <div class="min-w-0 flex-1">
                <p class="text-sm font-medium text-rose-700 dark:text-rose-300">{t('ask.requestFailed')}</p>
                <p class="mt-1 text-sm text-rose-600 dark:text-rose-400">{error}</p>
              </div>
            </div>
          {/if}

          <div bind:this={bottomAnchor} class="h-px w-full"></div>
        </div>
      {/if}
    </div>

    <div class="pointer-events-none sticky bottom-0 bg-gradient-to-t from-slate-50 via-slate-50/90 to-transparent px-4 pb-4 pt-8 dark:from-[#0d1117] dark:via-[#010409]/84">
      <div class="pointer-events-auto mx-auto max-w-4xl">
        <div class="ask-composer-shell rounded-[28px] border border-slate-200/70 bg-white/94 px-4 py-3 shadow-[0_12px_32px_rgba(15,23,42,0.08)] backdrop-blur dark:border-[#30363d]/70 dark:bg-[#161b22]/88 dark:shadow-[0_12px_32px_rgba(2,6,23,0.32)]">
          <textarea
            bind:this={composer}
            bind:value={input}
            rows="1"
            class="max-h-[220px] min-h-[26px] w-full resize-none bg-transparent text-[15px] leading-7 text-slate-900 outline-none placeholder:text-slate-400 dark:text-[#e6edf3] dark:placeholder:text-slate-500"
            placeholder={t('ask.placeholder')}
            on:input={resizeComposer}
            on:keydown={handleComposerKeydown}
          />

          <div class="mt-3 flex items-center justify-between gap-3 border-t border-slate-200/60 pt-2.5 dark:border-[#30363d]/60">
            <div class="flex min-w-0 items-center gap-2">
              <span bind:this={modelMeasureEl} class="invisible pointer-events-none absolute left-0 top-0 -z-10 whitespace-nowrap text-[11px] font-medium" aria-hidden="true"></span>
              <select
                bind:this={modelSelectEl}
                bind:value={selectedModelId}
                on:change={handleModelChange}
                class="h-8 max-w-[260px] cursor-pointer appearance-none rounded-lg border border-slate-200/80 bg-slate-100/90 px-3 pr-8 text-[11px] font-medium text-slate-700 outline-none transition hover:bg-slate-200/70 focus:ring-2 focus:ring-slate-300 dark:border-[#30363d]/80 dark:bg-[#21262d]/70 dark:text-[#adbac7] dark:hover:bg-[#30363d]/80 dark:focus:ring-primary-600"
                style="width: {modelSelectWidth}; background-image: url(&quot;data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%2394a3b8' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M6 9l6 6 6-6'/%3E%3C/svg%3E&quot;); background-repeat: no-repeat; background-position: right 10px center;"
                aria-label={t('ask.modelSelector')}
              >
                <option value={BASIC_ASSISTANT_MODEL_ID}>{t('ask.basicTemplate')}</option>
                {#each modelProfiles as profile}
                  <option value={profile.id}>{displayModelProfileName(profile) || t('ask.aiEnhanced')}</option>
                {/each}
              </select>

              {#if sending}
                <span class="shrink-0 text-[11px] text-slate-400 dark:text-[#636c76]">{t('ask.thinking')}</span>
              {:else}
                <button
                  type="button"
                  class="shrink-0 rounded-full px-2.5 py-1 text-[11px] text-slate-400 transition hover:bg-slate-100/80 hover:text-slate-700 dark:text-[#636c76] dark:hover:bg-[#21262d]/70 dark:hover:text-[#adbac7]"
                  on:click={clearConversation}
                  disabled={!hasConversation}
                >
                  {t('ask.clearing')}
                </button>
              {/if}
            </div>

            <button
              class="inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-gradient-to-br from-indigo-500 to-violet-500 text-white shadow-[0_6px_16px_rgba(79,70,229,0.32)] transition hover:scale-[1.04] hover:from-indigo-400 hover:to-violet-400 active:scale-95 disabled:cursor-not-allowed disabled:from-slate-300 disabled:to-slate-300 disabled:shadow-none dark:disabled:from-slate-700 dark:disabled:to-slate-700"
              on:click={() => submitQuestion()}
              disabled={sending || !input.trim()}
              aria-label={sending ? t('ask.sending') : t('ask.sendMessage')}
              title={sending ? t('ask.sending') : t('ask.sendMessage')}
            >
              {#if sending}
                <svg class="h-4 w-4 animate-spin" viewBox="0 0 24 24" fill="none">
                  <circle class="opacity-25" cx="12" cy="12" r="9" stroke="currentColor" stroke-width="2.5"></circle>
                  <path class="opacity-90" d="M21 12a9 9 0 0 0-9-9" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"></path>
                </svg>
              {:else}
                <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none">
                  <path d="M12 17V7" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"></path>
                  <path d="M8 11L12 7L16 11" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"></path>
                </svg>
              {/if}
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</div>
