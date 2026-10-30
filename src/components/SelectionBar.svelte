<script lang="ts">
  import { fly } from 'svelte/transition';
  import { fmtCount } from '../lib/format';
  import { app } from '../lib/store.svelte';
  import Icon from './Icon.svelte';

  const reduced = typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  const n = $derived(app.selected.size);
  const allSelected = $derived(n > 0 && n >= app.filtered.length);
</script>

{#if n > 0}
  <div class="bar" role="toolbar" aria-label="Selection" transition:fly={{ y: 16, duration: reduced ? 0 : 160 }}>
    <span class="count"><span class="num">{fmtCount(n)}</span> selected</span>
    {#if !allSelected}
      <button class="btn ghost" onclick={() => app.selectAllFiltered()}>
        Select all <span class="num">{fmtCount(app.filtered.length)}</span>
      </button>
    {/if}
    <button class="btn primary" onclick={() => app.exportIds([...app.selected])}><Icon name="export" size={16} />Export</button>
    <button class="btn ghost" onclick={() => app.clearSelection()} title="Clear selection (Esc)">Clear</button>
  </div>
{/if}

<style>
  .bar {
    position: absolute;
    left: 50%;
    bottom: 20px;
    translate: -50% 0;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 7px 7px 7px 18px;
    border-radius: var(--r-float);
    background: var(--panel);
    border: 1px solid var(--line);
    box-shadow: var(--shadow);
    z-index: 5;
    white-space: nowrap;
  }
  .count {
    font-weight: 600;
    margin-right: 8px;
  }
</style>
