<script lang="ts">
  import { fly } from 'svelte/transition';
  import { app } from '../lib/store.svelte';
  import Icon from './Icon.svelte';

  const reduced = typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;
</script>

<div class="toasts" role="status" aria-live="polite">
  {#each app.toasts as t (t.id)}
    <div class="toast" class:error={t.tone === 'error'} transition:fly={{ y: 12, duration: reduced ? 0 : 160 }}>
      <span class="msg">{t.message}</span>
      {#if t.action}
        <button
          class="btn"
          onclick={() => {
            t.action!.run();
            app.dismissToast(t.id);
          }}>{t.action.label}</button>
      {/if}
      <button class="btn ghost icon-only dismiss" onclick={() => app.dismissToast(t.id)} aria-label="Dismiss">
        <Icon name="x" size={14} />
      </button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 20px;
    bottom: 20px;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 8px;
    z-index: 50;
    pointer-events: none;
  }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: 420px;
    padding: 8px 8px 8px 16px;
    border-radius: var(--r-ctl);
    background: var(--panel);
    border: 1px solid var(--line);
    box-shadow: var(--shadow);
    font-size: var(--t-sm);
  }
  .toast.error {
    border-color: color-mix(in srgb, var(--danger) 60%, var(--line));
  }
  .toast.error .msg {
    color: var(--danger);
  }
  .msg {
    flex: 1;
    line-height: 1.35;
  }
  .dismiss {
    width: 28px;
    height: 28px;
  }
</style>
