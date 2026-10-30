<script lang="ts">
  import { untrack } from 'svelte';
  import { fmtCount, plural } from '../lib/format';
  import { app, TILE_MAX, TILE_MIN, type SortKey } from '../lib/store.svelte';
  import Icon from './Icon.svelte';

  let { searchEl = $bindable() }: { searchEl?: HTMLInputElement } = $props();

  // Debounced search: the input is local, the store gets it ~120 ms after typing stops.
  let text = $state(app.query);
  let timer: ReturnType<typeof setTimeout> | undefined;
  function onInput() {
    clearTimeout(timer);
    timer = setTimeout(() => (app.query = text), 120);
  }
  function clearSearch() {
    clearTimeout(timer);
    text = '';
    app.query = '';
    searchEl?.focus();
  }
  // Keep the box in sync when filters are cleared elsewhere.
  let lastQuery = app.query;
  $effect(() => {
    const q = app.query;
    if (q === lastQuery) return;
    lastQuery = q;
    if (q === '' && untrack(() => text) !== '') {
      clearTimeout(timer);
      text = '';
    }
  });

  function onSearchKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      if (text) clearSearch();
      else searchEl?.blur();
    } else if (e.key === 'ArrowDown' || e.key === 'Enter') {
      const tile = document.querySelector<HTMLElement>('.tile[tabindex="0"]');
      if (tile) {
        e.preventDefault();
        clearTimeout(timer);
        app.query = text;
        tile.focus();
      }
    }
  }

  const scanning = $derived(app.scan.phase === 'scanning');
  const total = $derived(app.visibleEntries.length);
  const shown = $derived(app.filtered.length);
  const progress = $derived(app.scan.total ? Math.min(1, app.scan.done / app.scan.total) : 0);

  const sourceNames = $derived.by(() => {
    const names = app.sources.map((s) => s.label);
    if (names.length <= 1) return names[0] ?? '';
    return `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]}`;
  });

  const status = $derived.by(() => {
    if (scanning) {
      return app.scan.total
        ? `Scanning ${fmtCount(app.scan.done)} of ${fmtCount(app.scan.total)} cache entries`
        : 'Looking for cache entries';
    }
    if (app.scan.phase === 'error') return 'The scan stopped early';
    if (app.detected === null) return 'Looking for Discord on this computer';
    if (!sourceNames) return 'No Discord cache found';
    const unread = app.scan.errors ? `. ${plural(app.scan.errors, 'entry', 'entries')} couldn't be read` : '';
    const from = app.lastDirs?.length === 1 ? app.lastDirs[0] : sourceNames;
    return `From ${from}${unread}`;
  });

  const announcement = $derived(
    app.scan.phase === 'done' && app.entries.length ? `Scan finished. Found ${plural(total, 'file')}.` : '',
  );

  const sorts: { value: SortKey; label: string }[] = [
    { value: 'posted', label: 'Newest posted' },
    { value: 'cached', label: 'Newest cached' },
    { value: 'size', label: 'Largest' },
    { value: 'name', label: 'Name' },
  ];
</script>

<header class="header">
  <div class="status-row">
    <div class="count">
      <span class="num big">{fmtCount(shown)}</span>
      <span class="unit">
        {#if shown !== total}of <span class="num">{fmtCount(total)}</span>{/if}
        {total === 1 && shown === total ? 'file' : 'files'}
      </span>
    </div>
    <p class="status num" title={app.sources.map((s) => s.path).join('\n')}>{status}</p>
    {#if app.scan.diagnostics && !scanning}
    <button
      class="btn ghost rescan"
      onclick={() => app.copyDiagnostics()}
      title="Copy counts from the last scan (no file names or links) to include in a bug report">
      <Icon name="copy" size={16} />Copy scan details
    </button>
    {/if}
    {#if app.sources.length}
    <button class="btn ghost rescan" onclick={() => app.rescan()} title="Scan the cache again">
      <Icon name="refresh" size={16} />Rescan
    </button>
    {/if}
  </div>

  <div class="toolbar">
    <label class="search">
      <Icon name="search" size={16} />
      <span class="visually-hidden">Search</span>
      <input
        bind:this={searchEl}
        bind:value={text}
        oninput={onInput}
        onkeydown={onSearchKey}
        type="search"
        placeholder="Search files, links, channel IDs…"
        spellcheck="false"
        autocomplete="off" />
      {#if text}
        <button class="clear" onclick={clearSearch} aria-label="Clear search"><Icon name="x" size={14} /></button>
      {:else}
        <kbd aria-hidden="true">/</kbd>
      {/if}
    </label>

    <label class="sort">
      <span class="visually-hidden">Sort by</span>
      <select class="field" value={app.sort} onchange={(e) => app.setSort(e.currentTarget.value as SortKey)}>
        {#each sorts as s (s.value)}<option value={s.value}>{s.label}</option>{/each}
      </select>
    </label>

    <label class="size" title="Tile size">
      <span class="size-label">Size</span>
      <input
        type="range"
        min={TILE_MIN}
        max={TILE_MAX}
        step="4"
        value={app.tileSize}
        oninput={(e) => app.setTileSize(Number(e.currentTarget.value))}
        aria-label="Tile size" />
    </label>

    <button
      class="btn toggle"
      aria-pressed={app.groupSizes}
      onclick={() => app.setGroupSizes(!app.groupSizes)}
      title="Show one copy of images Discord cached at several sizes (keeps the largest)">
      <Icon name="layers" size={16} /><span class="label">Group sizes</span>
    </button>
  </div>
  <div
    class="progress"
    class:on={scanning}
    role="progressbar"
    aria-label="Scan progress"
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={Math.round(progress * 100)}
    aria-hidden={!scanning}>
    <div class="bar" style:transform="scaleX({progress})"></div>
  </div>
  <p class="visually-hidden" aria-live="polite">{announcement}</p>
</header>

<style>
  .header {
    position: relative;
    container-type: inline-size;
    flex: none;
    background: var(--ink);
  }
  .status-row {
    position: relative;
    display: flex;
    align-items: baseline;
    gap: 16px;
    padding: 18px 20px 12px;
    min-width: 0;
  }
  .count {
    display: flex;
    align-items: baseline;
    gap: 6px;
    flex: none;
  }
  .big {
    font-size: var(--t-xl);
    font-weight: 650;
    letter-spacing: -0.01em;
    font-variation-settings: 'opsz' 48;
  }
  .unit {
    color: var(--muted);
    font-size: var(--t-md);
  }
  .status {
    margin: 0;
    color: var(--muted);
    font-size: var(--t-sm);
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .rescan {
    align-self: center;
    margin: -6px -8px -6px 0;
  }
  .progress {
    position: absolute;
    left: 0;
    right: 0;
    bottom: -1px;
    height: 2px;
    z-index: 2;
    opacity: 0;
    transition: opacity 400ms;
  }
  .progress.on {
    opacity: 1;
  }
  .bar {
    height: 100%;
    background: var(--accent);
    transform-origin: left;
    transition: transform 200ms linear;
  }

  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 10px;
    padding: 4px 20px 12px;
    border-bottom: 1px solid var(--line);
  }

  .search {
    position: relative;
    flex: 1 1 280px;
    max-width: 560px;
    min-width: 160px;
    display: flex;
    align-items: center;
    height: 36px;
    border-radius: var(--r-ctl);
    border: 1px solid var(--line);
    background: var(--panel);
    color: var(--muted);
    padding: 0 8px 0 11px;
    gap: 8px;
    cursor: text;
  }
  .search:focus-within {
    border-color: var(--accent);
    outline: 1px solid var(--accent);
  }
  .search input {
    flex: 1;
    min-width: 0;
    height: 100%;
    border: 0;
    background: none;
    outline: none;
    color: var(--text);
    font-size: var(--t-md);
  }
  .search input::placeholder {
    color: var(--muted);
  }
  .search input::-webkit-search-cancel-button {
    display: none;
  }
  kbd {
    font: inherit;
    font-size: var(--t-xs);
    font-weight: 600;
    min-width: 20px;
    height: 20px;
    display: grid;
    place-items: center;
    border-radius: 5px;
    border: 1px solid var(--line);
    color: var(--muted);
  }
  .clear {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .clear:hover {
    color: var(--text);
    background: var(--raised);
  }

  .size {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--muted);
    margin-left: auto;
  }
  .size-label {
    font-size: var(--t-sm);
  }
  input[type='range'] {
    width: 96px;
    accent-color: var(--accent);
    cursor: pointer;
  }

  .toggle[aria-pressed='true'] {
    color: var(--text);
    border-color: color-mix(in srgb, var(--accent) 55%, var(--line));
    background: var(--accent-soft);
  }
  .toggle[aria-pressed='false'] {
    color: var(--muted);
    background: transparent;
  }

  @container (width < 760px) {
    .toggle .label,
    .rescan {
      font-size: 0;
      gap: 0;
    }
    .toggle {
      width: 34px;
      padding: 0;
      justify-content: center;
    }
    input[type='range'] {
      width: 64px;
    }
  }

  @container (width < 600px) {
    .search {
      flex-basis: 100%;
      max-width: none;
    }
  }
</style>
