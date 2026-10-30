<script lang="ts">
  import Drawer from './components/Drawer.svelte';
  import Grid from './components/Grid.svelte';
  import Header from './components/Header.svelte';
  import LockedNotice from './components/LockedNotice.svelte';
  import SelectionBar from './components/SelectionBar.svelte';
  import Sidebar from './components/Sidebar.svelte';
  import Toasts from './components/Toasts.svelte';
  import { app } from './lib/store.svelte';

  let searchEl: HTMLInputElement | undefined = $state();

  function isTyping(target: EventTarget | null) {
    return target instanceof HTMLElement && target.closest('input, textarea, select, [contenteditable="true"]') !== null;
  }

  function onKey(e: KeyboardEvent) {
    if (e.defaultPrevented) return;
    const typing = isTyping(e.target);
    if (e.key === '/' && !typing && !e.ctrlKey && !e.metaKey && !e.altKey) {
      e.preventDefault();
      searchEl?.focus();
      searchEl?.select();
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a' && !typing) {
      e.preventDefault();
      app.selectAllFiltered();
    } else if (e.key === 'Escape' && !typing) {
      if (app.openId != null) {
        app.openId = null;
        document.querySelector<HTMLElement>('.tile[tabindex="0"]')?.focus();
      } else {
        app.clearSelection();
      }
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="shell">
  <Sidebar />
  <main>
    <Header bind:searchEl />
    {#if app.scan.phase === 'done' && app.scan.lockedSources.length}
      <LockedNotice sources={app.scan.lockedSources} />
    {/if}
    <div class="stage">
      <Grid />
      <SelectionBar />
    </div>
  </main>
  {#if app.openEntry}
    <Drawer entry={app.openEntry} />
  {/if}
</div>
<Toasts />

<style>
  .shell {
    position: relative;
    display: flex;
    height: 100%;
    overflow: hidden;
  }
  main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .stage {
    position: relative;
    flex: 1;
    min-height: 0;
  }
</style>
