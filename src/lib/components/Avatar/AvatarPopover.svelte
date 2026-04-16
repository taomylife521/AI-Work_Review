<script>
  import { formatBubbleMessage } from './bubbleMessage.js';

  export let bubble = null;
  export let onClose = () => {};

  $: bubbleMessage = formatBubbleMessage(bubble?.message);
  $: panelStyle =
    bubble?.tone === 'success'
      ? 'background: linear-gradient(180deg, rgba(236, 253, 245, 0.98), rgba(209, 250, 229, 0.95)); color: rgb(6, 78, 59); border-color: rgba(167, 243, 208, 0.96); backdrop-filter: blur(14px) saturate(1.06);'
      : 'background: rgba(255, 255, 255, 0.96); color: rgb(15, 23, 42); border-color: rgba(226, 232, 240, 0.96); backdrop-filter: blur(14px) saturate(1.06);';
  $: innerPanelStyle = 'border-color: rgba(255, 255, 255, 0.74);';
  $: tailStyle =
    bubble?.tone === 'success'
      ? 'background: linear-gradient(180deg, rgba(236, 253, 245, 0.98), rgba(209, 250, 229, 0.95)); border-color: rgba(167, 243, 208, 0.96);'
      : 'background: rgba(255, 255, 255, 0.96); border-color: rgba(226, 232, 240, 0.96);';
  $: tailDotStyle =
    bubble?.tone === 'success'
      ? 'background: rgba(236, 253, 245, 0.98);'
      : 'background: rgba(255, 255, 255, 0.94);';
</script>

{#if bubble}
  <div class="absolute inset-0 z-20 overflow-visible pointer-events-none">
    <div class="absolute" style="right: 10%; top: 6%;">
      <div class="relative overflow-visible">
        <div
          class="pointer-events-auto relative rounded-[20px] border shadow-[0_8px_22px_rgba(15,23,42,0.12),0_2px_8px_rgba(15,23,42,0.08)]"
          style="width: fit-content; max-width: min(62vw, 228px); min-width: 118px; padding: 12px 14px 12px 14px; {panelStyle}"
        >
          {#if bubble?.persistent}
            <button
              type="button"
              class="absolute inset-0 rounded-[20px]"
              aria-label="关闭提醒"
              on:click={onClose}
            ></button>
          {/if}
          <div
            class="pointer-events-none absolute inset-[1px] rounded-[19px] border"
            style={innerPanelStyle}
          ></div>
          {#if bubble?.persistent}
            <button
              type="button"
              class="absolute right-2 top-2 z-10 inline-flex h-5 w-5 items-center justify-center rounded-full text-slate-400 transition hover:bg-slate-900/6 hover:text-slate-700"
              aria-label="关闭提醒"
              on:click={onClose}
            >
              ×
            </button>
          {/if}
          <div
            class="pointer-events-none relative text-[13px] font-medium leading-[1.45] tracking-[0.01em]"
            class:pr-6={bubble?.persistent}
            style="display: inline-flex; word-break: break-word; overflow-wrap: anywhere; white-space: pre-wrap;"
          >
            {bubbleMessage}
          </div>
        </div>
        <div
          class="bubble-tail absolute left-[20px] top-[calc(100%-7px)] h-[16px] w-[16px] rotate-45 rounded-[4px] border shadow-[0_6px_16px_rgba(15,23,42,0.08)]"
          style={tailStyle}
        ></div>
        <div
          class="absolute left-[30px] top-[calc(100%-2px)] h-[10px] w-[10px] rounded-full shadow-[0_4px_12px_rgba(15,23,42,0.08)]"
          style={tailDotStyle}
        ></div>
      </div>
    </div>
  </div>
{/if}
