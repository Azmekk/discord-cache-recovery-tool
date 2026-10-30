<script lang="ts">
  import { fmtCount, kindLabels, type FormatKey } from '../lib/format';
  import { app, type DateRange, type KindFilter } from '../lib/store.svelte';
  import Icon, { type IconName } from './Icon.svelte';

  /** Only matters in the narrow layout, where the sidebar is an icon rail that can expand over the grid. */
  let expanded = $state(false);

  const baseKinds: KindFilter[] = ['all', 'attachment', 'gif', 'emoji', 'sticker', 'avatar', 'server', 'embed', 'unknown'];
  const kinds = $derived<KindFilter[]>(app.showInterface ? [...baseKinds, 'interface'] : baseKinds);

  const formats: { key: FormatKey; label: string; icon: IconName }[] = [
    { key: 'image', label: 'Images', icon: 'image' },
    { key: 'gif', label: 'GIFs', icon: 'gif' },
    { key: 'video', label: 'Videos', icon: 'video' },
    { key: 'audio', label: 'Audio', icon: 'audio' },
  ];

  const ranges: { value: DateRange; label: string }[] = [
    { value: 'any', label: 'Any time' },
    { value: 'today', label: 'Today' },
    { value: 'week', label: 'Past week' },
    { value: 'month', label: 'Past month' },
    { value: 'year', label: 'Past year' },
    { value: 'custom', label: 'Custom' },
  ];

  const countOf = (k: KindFilter) => (k === 'all' ? app.base.length : (app.kindCounts[k] ?? 0));
  const labelOf = (k: KindFilter) => (k === 'all' ? 'All' : kindLabels[k]);

  function pickKind(k: KindFilter) {
    app.kind = k;
    expanded = false;
  }

  function onKey(e: KeyboardEvent) {
    if (expanded && e.key === 'Escape') {
      e.stopPropagation();
      expanded = false;
    }
  }
</script>

{#if expanded}
  <button class="scrim" aria-label="Close filters" tabindex="-1" onclick={() => (expanded = false)}></button>
{/if}

<div class="slot">
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<nav class="sidebar" class:expanded aria-label="Filters" onkeydown={onKey}>
  <div class="top">
    <h1 class="title">Discord Cache Recovery</h1>
    <button
      class="btn ghost icon-only rail-toggle"
      aria-expanded={expanded}
      aria-label={expanded ? 'Collapse filters' : 'Show all filters'}
      title={expanded ? 'Collapse filters' : 'Show all filters'}
      onclick={() => (expanded = !expanded)}>
      <Icon name="panel" />
    </button>
  </div>

  <div class="scroll">
    <section aria-label="Kind">
      <ul class="kinds">
        {#each kinds as k (k)}
          {@const n = countOf(k)}
          <li>
            <button
              class="kind"
              class:active={app.kind === k}
              class:zero={n === 0}
              aria-pressed={app.kind === k}
              title={labelOf(k)}
              onclick={() => pickKind(k)}>
              <Icon name={k as IconName} size={17} />
              <span class="label">{labelOf(k)}</span>
              <span class="count num">{fmtCount(n)}</span>
            </button>
          </li>
        {/each}
      </ul>
      <label class="switch">
        <input type="checkbox" checked={app.showInterface} onchange={(e) => app.setInterfaceShown(e.currentTarget.checked)} />
        <span class="track" aria-hidden="true"></span>
        <span class="switch-label">Show Discord's own interface images</span>
      </label>
    </section>

    <section class="extra">
      <h2>Format</h2>
      <div class="chips">
        {#each formats as f (f.key)}
          <button class="chip" aria-pressed={app.formats.has(f.key)} onclick={() => app.toggleFormat(f.key)}>
            {f.label}
          </button>
        {/each}
      </div>
    </section>

    <section class="extra">
      <h2><label for="date-range">Date</label></h2>
      <select
        id="date-range"
        class="field wide"
        title="Uses the date a file was posted, or when Discord saved it"
        value={app.dateRange}
        onchange={(e) => (app.dateRange = e.currentTarget.value as DateRange)}>
        {#each ranges as r (r.value)}<option value={r.value}>{r.label}</option>{/each}
      </select>
      {#if app.dateRange === 'custom'}
        <div class="custom">
          <label>
            <span>From</span>
            <input class="field" type="date" bind:value={app.customFrom} max={app.customTo || undefined} />
          </label>
          <label>
            <span>To</span>
            <input class="field" type="date" bind:value={app.customTo} min={app.customFrom || undefined} />
          </label>
        </div>
      {/if}
    </section>

    <section class="extra">
      <h2>Sources</h2>
      {#if app.sources.length}
        <ul class="sources">
          {#each app.sources as s (s.path)}
            <li>
              <button
                class="source"
                class:active={app.sourceFilter === s.label}
                aria-pressed={app.sourceFilter === s.label}
                title={s.path}
                onclick={() => (app.sourceFilter = app.sourceFilter === s.label ? null : s.label)}>
                <Icon name="drive" size={16} />
                <span class="label">{s.label}</span>
                <span class="count num">{fmtCount(app.sourceCounts[s.label] ?? 0)}</span>
              </button>
            </li>
          {/each}
        </ul>
      {:else if app.detected !== null}
        <p class="hint">No Discord install found.</p>
      {/if}
      {#if app.lastDirs && app.detected?.length}
        <button class="btn ghost choose" onclick={() => app.startScan(null)}>
          <Icon name="refresh" size={16} />Back to Discord's cache
        </button>
      {/if}
      <button class="btn ghost choose" onclick={() => app.chooseCacheFolder()}>
        <Icon name="folder" size={16} />Choose a cache folder…
      </button>
    </section>
  </div>
</nav>
</div>

<style>
  .slot {
    position: relative;
    flex: none;
    display: flex;
    min-height: 0;
  }
  .sidebar {
    width: var(--sidebar);
    flex: none;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border-right: 1px solid var(--line);
    min-height: 0;
    z-index: 20;
  }
  .top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    padding: 16px 16px 14px 20px;
  }
  .title {
    margin: 0;
    font-size: var(--t-xl);
    font-weight: 700;
    line-height: 1.02;
    letter-spacing: -0.025em;
    font-variation-settings: 'opsz' 96;
    text-wrap: balance;
  }
  .rail-toggle {
    display: none;
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 0 12px 20px;
  }
  section + section {
    margin-top: 22px;
  }
  h2 {
    margin: 0 0 8px 8px;
    font-size: var(--t-sm);
    font-weight: 600;
    color: var(--muted);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .kind,
  .source {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 34px;
    padding: 0 8px;
    border: 0;
    border-radius: var(--r-ctl);
    background: none;
    color: var(--text);
    font-size: var(--t-md);
    text-align: left;
    cursor: pointer;
  }
  .kind :global(.icon),
  .source :global(.icon) {
    color: var(--muted);
  }
  .kind:hover,
  .source:hover {
    background: var(--raised);
  }
  .kind.active,
  .source.active {
    background: var(--raised);
    box-shadow: inset 2px 0 0 var(--accent);
    font-weight: 600;
  }
  .kind.active :global(.icon),
  .source.active :global(.icon) {
    color: var(--accent);
  }
  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    color: var(--muted);
    font-size: var(--t-sm);
    font-weight: 450;
  }
  .kind.zero .label {
    color: var(--muted);
  }

  .switch {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    margin: 10px 8px 0;
    color: var(--muted);
    font-size: var(--t-sm);
    line-height: 1.3;
    cursor: pointer;
  }
  .switch input {
    position: absolute;
    opacity: 0;
    width: 1px;
    height: 1px;
  }
  .track {
    flex: none;
    position: relative;
    width: 28px;
    height: 16px;
    border-radius: 8px;
    background: var(--line);
    margin-top: 1px;
    transition: background-color 120ms;
  }
  .track::after {
    content: '';
    position: absolute;
    left: 2px;
    top: 2px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--text);
    transition: transform 120ms var(--ease);
  }
  .switch input:checked + .track {
    background: var(--accent);
  }
  .switch input:checked + .track::after {
    transform: translateX(12px);
    background: var(--accent-ink);
  }
  .switch input:focus-visible + .track {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 0 4px;
  }
  .chip {
    height: 30px;
    padding: 0 12px;
    border-radius: var(--r-ctl);
    border: 1px solid var(--line);
    background: none;
    color: var(--text);
    font-size: var(--t-sm);
    cursor: pointer;
  }
  .chip:hover {
    background: var(--raised);
  }
  .chip[aria-pressed='true'] {
    border-color: var(--accent);
    background: var(--accent-soft);
    font-weight: 600;
  }

  .wide {
    width: calc(100% - 8px);
    margin: 0 4px;
  }
  .custom {
    display: grid;
    gap: 6px;
    margin: 8px 4px 0;
  }
  .custom label {
    display: grid;
    grid-template-columns: 40px 1fr;
    align-items: center;
    font-size: var(--t-sm);
    color: var(--muted);
  }
  .custom .field {
    min-width: 0;
  }
  .hint {
    margin: 8px 8px 0;
    font-size: var(--t-xs);
    color: var(--muted);
    line-height: 1.4;
  }
  .choose {
    margin-top: 4px;
    padding: 0 8px;
    width: 100%;
    gap: 10px;
    font-size: var(--t-md);
    font-weight: 450;
  }

  .scrim {
    display: none;
  }

  /* Narrow windows: icon rail that expands over the grid. */
  @media (max-width: 1099px) {
    .slot {
      width: 60px;
    }
    .sidebar:not(.expanded) {
      width: 60px;
    }
    .sidebar:not(.expanded) .title,
    .sidebar:not(.expanded) .label,
    .sidebar:not(.expanded) .count,
    .sidebar:not(.expanded) .switch,
    .sidebar:not(.expanded) .extra {
      display: none;
    }
    .sidebar:not(.expanded) .top {
      padding: 12px 0 10px;
      justify-content: center;
    }
    .sidebar:not(.expanded) .scroll {
      padding: 0 8px 16px;
    }
    .sidebar:not(.expanded) .kind {
      justify-content: center;
      padding: 0;
      height: 38px;
    }
    .rail-toggle {
      display: inline-flex;
    }
    .sidebar.expanded {
      position: absolute;
      left: 0;
      top: 0;
      bottom: 0;
      box-shadow: var(--shadow);
    }
    .sidebar.expanded .top {
      padding-right: 10px;
    }
    .scrim {
      display: block;
      position: fixed;
      inset: 0;
      z-index: 19;
      border: 0;
      background: var(--scrim);
      opacity: 0.5;
    }
  }
</style>
