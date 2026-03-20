<script>
  export let title = '';
  export let value = '';
  export let icon = '';
  export let color = 'indigo';
  export let href = null;

  const iconColors = {
    indigo: 'text-indigo-500 dark:text-indigo-400',
    emerald: 'text-emerald-500 dark:text-emerald-400',
    amber: 'text-amber-500 dark:text-amber-400',
    rose: 'text-rose-500 dark:text-rose-400',
    blue: 'text-blue-500 dark:text-blue-400',
    violet: 'text-violet-500 dark:text-violet-400',
  };

  const iconBgs = {
    indigo: 'bg-indigo-50 dark:bg-indigo-950/40',
    emerald: 'bg-emerald-50 dark:bg-emerald-950/40',
    amber: 'bg-amber-50 dark:bg-amber-950/40',
    rose: 'bg-rose-50 dark:bg-rose-950/40',
    blue: 'bg-blue-50 dark:bg-blue-950/40',
    violet: 'bg-violet-50 dark:bg-violet-950/40',
  };
</script>

{#if href}
  <a
    {href}
    class="block min-h-[116px] p-5 rounded-2xl bg-white dark:bg-slate-800/80 border border-slate-100 dark:border-slate-700/60
           transition-all duration-200 hover:shadow-md hover:-translate-y-0.5 cursor-pointer group"
  >
    <div class="flex h-full items-center justify-between gap-4">
      <div class="min-w-0 flex-1">
        <span class="text-[13px] font-medium text-slate-400 dark:text-slate-500 leading-none">{title}</span>
        <p class="mt-6 text-[2.15rem] font-semibold text-slate-800 dark:text-white tracking-tight leading-none">{value}</p>
      </div>
      <div class="w-11 h-11 rounded-2xl border border-white/80 dark:border-slate-700/80 shadow-sm {iconBgs[color]} flex items-center justify-center {iconColors[color]} shrink-0">
        {#if icon === 'duration'}
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M12 7v5l3 2m6-2a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        {:else if icon === 'focus'}
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M4 10V7a3 3 0 013-3h3m4 0h3a3 3 0 013 3v3m0 4v3a3 3 0 01-3 3h-3m-4 0H7a3 3 0 01-3-3v-3" />
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M9 12h6" />
          </svg>
        {:else if icon === 'browser'}
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M3 12h18M12 3a15.3 15.3 0 014 9 15.3 15.3 0 01-4 9 15.3 15.3 0 01-4-9 15.3 15.3 0 014-9Z" />
            <circle cx="12" cy="12" r="9" stroke-width="1.8" />
          </svg>
        {:else if icon === 'apps'}
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <rect x="4" y="4" width="7" height="7" rx="2" stroke-width="1.8" />
            <rect x="13" y="4" width="7" height="7" rx="2" stroke-width="1.8" />
            <rect x="4" y="13" width="7" height="7" rx="2" stroke-width="1.8" />
            <rect x="13" y="13" width="7" height="7" rx="2" stroke-width="1.8" />
          </svg>
        {:else}
          {icon}
        {/if}
      </div>
    </div>
  </a>
{:else}
  <div class="min-h-[116px] p-5 rounded-2xl bg-white dark:bg-slate-800/80 border border-slate-100 dark:border-slate-700/60">
    <div class="flex h-full items-center justify-between gap-4">
      <div class="min-w-0 flex-1">
        <span class="text-[13px] font-medium text-slate-400 dark:text-slate-500 leading-none">{title}</span>
        <p class="mt-6 text-[2.15rem] font-semibold text-slate-800 dark:text-white tracking-tight leading-none">{value}</p>
      </div>
      <div class="w-11 h-11 rounded-2xl border border-white/80 dark:border-slate-700/80 shadow-sm {iconBgs[color]} flex items-center justify-center {iconColors[color]} shrink-0">
        {#if icon === 'duration'}
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M12 7v5l3 2m6-2a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        {:else if icon === 'focus'}
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M4 10V7a3 3 0 013-3h3m4 0h3a3 3 0 013 3v3m0 4v3a3 3 0 01-3 3h-3m-4 0H7a3 3 0 01-3-3v-3" />
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M9 12h6" />
          </svg>
        {:else if icon === 'browser'}
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M3 12h18M12 3a15.3 15.3 0 014 9 15.3 15.3 0 01-4 9 15.3 15.3 0 01-4-9 15.3 15.3 0 014-9Z" />
            <circle cx="12" cy="12" r="9" stroke-width="1.8" />
          </svg>
        {:else if icon === 'apps'}
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <rect x="4" y="4" width="7" height="7" rx="2" stroke-width="1.8" />
            <rect x="13" y="4" width="7" height="7" rx="2" stroke-width="1.8" />
            <rect x="4" y="13" width="7" height="7" rx="2" stroke-width="1.8" />
            <rect x="13" y="13" width="7" height="7" rx="2" stroke-width="1.8" />
          </svg>
        {:else}
          {icon}
        {/if}
      </div>
    </div>
  </div>
{/if}
