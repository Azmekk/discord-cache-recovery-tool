<script lang="ts">
  import { app } from '../lib/store.svelte';
  import Icon from './Icon.svelte';

  type State = 'reading' | 'no-install' | 'empty-cache' | 'error' | 'no-match';

  const state: State = $derived.by(() => {
    if (app.entries.length > 0) return 'no-match';
    if (app.scan.phase === 'scanning' || app.scan.phase === 'idle') return 'reading';
    if (app.scan.phase === 'error') return 'error';
    if (app.sources.length === 0) return 'no-install';
    return 'empty-cache';
  });

  const onlyHidden = $derived(
    state === 'no-match' && !app.filtersActive && !app.showInterface && app.entries.every((e) => e.kind === 'interface'),
  );
</script>

<div class="empty" class:quiet={state === 'reading'} aria-live="polite">
  {#if state === 'reading'}
    <p class="lede">Reading Discord's cache. Files show up here as they're found.</p>
  {:else if state === 'no-install'}
    <h2>Point us at a Discord cache</h2>
    <p>
      We couldn't find Discord, Discord PTB or Discord Canary in the usual places on this computer. If Discord lives
      somewhere else, or you have a cache folder copied from another PC, choose its <em>Cache_Data</em> folder.
    </p>
    <div class="actions">
      <button class="btn primary" onclick={() => app.chooseCacheFolder()}>
        <Icon name="folder" size={16} />Choose a cache folder…
      </button>
    </div>
  {:else if state === 'empty-cache'}
    <h2>This cache has no media left</h2>
    <p>
      Discord trims its cache over time, and clearing it (or reinstalling Discord) empties it. If you use another Discord
      build, or have a backup of an older cache folder, scan that instead.
    </p>
    <div class="actions">
      <button class="btn primary" onclick={() => app.chooseCacheFolder()}>
        <Icon name="folder" size={16} />Choose a cache folder…
      </button>
      <button class="btn" onclick={() => app.startScan(null)}><Icon name="refresh" size={16} />Scan again</button>
    </div>
  {:else if state === 'error'}
    <h2>The scan stopped early</h2>
    <p>{app.scan.message || 'Something interrupted reading the cache.'} Close Discord and try again, or choose a folder.</p>
    <div class="actions">
      <button class="btn primary" onclick={() => app.startScan(null)}><Icon name="refresh" size={16} />Scan again</button>
      <button class="btn" onclick={() => app.chooseCacheFolder()}><Icon name="folder" size={16} />Choose a cache folder…</button>
    </div>
  {:else if onlyHidden}
    <h2>Only Discord's own interface images are here</h2>
    <p>These are icons and artwork from Discord itself, not anything that was shared with you.</p>
    <div class="actions">
      <button class="btn" onclick={() => app.setInterfaceShown(true)}>Show them anyway</button>
    </div>
  {:else}
    <h2>Nothing matches these filters</h2>
    <p>Try fewer words, a wider date range, or another kind of file.</p>
    <div class="actions">
      <button class="btn primary" onclick={() => app.clearFilters()}>Clear filters</button>
    </div>
  {/if}
</div>

<style>
  .empty {
    padding: 56px 48px;
    max-width: 560px;
  }
  h2 {
    margin: 0 0 12px;
    font-size: var(--t-2xl);
    font-weight: 650;
    line-height: 1.1;
    letter-spacing: -0.015em;
    font-variation-settings: 'opsz' 96;
    text-wrap: balance;
  }
  p {
    margin: 0;
    color: var(--muted);
    font-size: var(--t-lg);
    line-height: 1.5;
    text-wrap: pretty;
  }
  em {
    font-style: normal;
    color: var(--text);
    font-weight: 550;
  }
  .lede {
    font-size: var(--t-md);
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 24px;
    flex-wrap: wrap;
  }
</style>
