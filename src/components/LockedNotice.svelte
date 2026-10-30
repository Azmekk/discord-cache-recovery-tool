<script lang="ts">
  import { app } from '../lib/store.svelte';
  import Icon from './Icon.svelte';

  let { sources }: { sources: string[] } = $props();

  const which = $derived(sources.length === 1 ? sources[0] : 'Discord');
</script>

<div class="notice" role="status">
  <div class="text">
    <p class="title">Close {which} to see file names, links and dates</p>
    <p class="body">
      {which} keeps its cache index locked while it runs, so these files are listed without their
      details. Quit it completely, including from the system tray, then rescan.
    </p>
  </div>
  <button class="btn primary" onclick={() => app.rescan()}>
    <Icon name="refresh" size={16} />Rescan
  </button>
</div>

<style>
  .notice {
    flex: none;
    display: flex;
    align-items: center;
    gap: 16px;
    margin: 12px 20px 0;
    padding: 12px 14px 12px 16px;
    border-radius: var(--r-ctl);
    border: 1px solid color-mix(in srgb, var(--accent) 45%, var(--line));
    background: var(--accent-soft);
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .title {
    margin: 0;
    font-size: var(--t-md);
    font-weight: 620;
    color: var(--text);
  }
  .body {
    margin: 2px 0 0;
    font-size: var(--t-sm);
    color: var(--muted);
    max-width: 72ch;
  }
</style>
