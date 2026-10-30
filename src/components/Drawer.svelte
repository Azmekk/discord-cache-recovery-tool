<script lang="ts">
  import { fly } from 'svelte/transition';
  import * as api from '../lib/api';
  import { fmtBytes, fmtDateTime, formatOf, kindSingular } from '../lib/format';
  import { app, errorText } from '../lib/store.svelte';
  import type { Entry } from '../lib/types';
  import Icon from './Icon.svelte';

  let { entry }: { entry: Entry } = $props();

  const reduced = typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;

  const fmt = $derived(formatOf(entry));
  const src = $derived(api.mediaUrl(entry));
  const poster = $derived(fmt === 'video' ? api.posterUrl(entry) : null);
  const link = $derived(entry.originalUrl ?? entry.url);
  const contain = $derived(entry.kind === 'emoji' || entry.kind === 'sticker' || entry.kind === 'interface');
  let failed = $state(false);
  $effect(() => {
    void entry.id;
    failed = false;
  });

  let labelDraft = $state('');
  $effect(() => {
    labelDraft = entry.channelId ? (app.channelLabels[entry.channelId] ?? '') : '';
  });
  function saveLabel() {
    if (entry.channelId) app.setChannelLabel(entry.channelId, labelDraft);
  }

  async function copy(text: string, what: string) {
    try {
      await navigator.clipboard.writeText(text);
      app.toast(`${what} copied`);
    } catch (err) {
      app.toast(`Couldn't copy: ${errorText(err)}`, 'error');
    }
  }

  async function openOriginal() {
    if (!link) return;
    try {
      await api.openUrl(link);
    } catch (err) {
      app.toast(`Couldn't open the link: ${errorText(err)}`, 'error');
    }
  }

  function close() {
    app.openId = null;
    document.querySelector<HTMLElement>('.tile[tabindex="0"]')?.focus();
  }
</script>

<aside class="drawer" aria-label="File details" transition:fly={{ x: 32, duration: reduced ? 0 : 180, opacity: 0 }}>
  <div class="bar">
    <span class="kind">{kindSingular[entry.kind]}</span>
    <button class="btn ghost icon-only" onclick={close} aria-label="Close details" title="Close (Esc)">
      <Icon name="x" />
    </button>
  </div>

  <div class="scroll">
    {#key entry.id}
      <div class="preview" class:checker={fmt !== 'audio' && fmt !== 'other'} class:contain>
        {#if fmt === 'video' && src}
          <!-- svelte-ignore a11y_media_has_caption -->
          <video {src} poster={poster ?? undefined} controls loop={entry.kind === 'gif'} autoplay={entry.kind === 'gif'} muted={entry.kind === 'gif'} playsinline></video>
        {:else if fmt === 'video' && poster}
          <img src={poster} alt="" />
        {:else if fmt === 'audio'}
          <div class="glyph">
            <Icon name="audio" size={36} />
            {#if src}<audio {src} controls></audio>{/if}
          </div>
        {:else if fmt === 'other' || failed || !src}
          <div class="glyph">
            <Icon name="unknown" size={36} />
            <span>No preview for {entry.ext.toUpperCase()} files</span>
          </div>
        {:else}
          <img {src} alt={entry.fileName} onerror={() => (failed = true)} />
        {/if}
      </div>
    {/key}

    <h2 class="name">{entry.fileName}</h2>

    <div class="actions">
      <button class="btn primary" onclick={() => app.exportIds([entry.id])}><Icon name="export" size={16} />Export…</button>
      {#if link}
        <button class="btn" onclick={() => copy(link, 'Link')}><Icon name="copy" size={16} />Copy link</button>
      {/if}
    </div>

    <dl class="meta">
      <dt>Kind</dt>
      <dd>{kindSingular[entry.kind]}</dd>
      <dt>Type</dt>
      <dd>{entry.mime}</dd>
      <dt>Size</dt>
      <dd class="num">{fmtBytes(entry.size)}</dd>
      <dt>Posted</dt>
      <dd class="num">{entry.postedAt == null ? 'Not known' : fmtDateTime(entry.postedAt)}</dd>
      <dt>Cached</dt>
      <dd class="num">{fmtDateTime(entry.cachedAt)}</dd>
      {#if entry.channelId}
        <dt>Channel</dt>
        <dd>
          <div class="inline">
            <span class="num">{entry.channelId}</span>
            <button class="mini" onclick={() => copy(entry.channelId!, 'Channel ID')}>
              <Icon name="copy" size={13} />Copy channel ID
            </button>
          </div>
          <input
            class="field label-input"
            bind:value={labelDraft}
            onchange={saveLabel}
            onkeydown={(e) => {
              if (e.key === 'Enter') e.currentTarget.blur();
            }}
            placeholder="Add a label, like “Movie night”"
            aria-label="Label for this channel" />
        </dd>
      {/if}
      {#if entry.guildId}
        <dt>Server</dt>
        <dd class="num">{entry.guildId}</dd>
      {/if}
      <dt>Source</dt>
      <dd>{entry.source}</dd>
      {#if link}
        <dt>Link</dt>
        <dd>
          <span class="link" title={link}>{link}</span>
          <button class="mini" onclick={openOriginal}><Icon name="external" size={13} />Open original in browser</button>
        </dd>
      {/if}
    </dl>
  </div>
</aside>

<style>
  .drawer {
    width: var(--drawer);
    flex: none;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border-left: 1px solid var(--line);
    min-height: 0;
  }
  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 10px 6px 20px;
  }
  .kind {
    font-size: var(--t-sm);
    color: var(--muted);
    font-weight: 550;
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 4px 20px 28px;
  }
  .preview {
    position: relative;
    display: grid;
    place-items: center;
    min-height: 200px;
    max-height: 340px;
    border-radius: var(--r-tile);
    overflow: hidden;
    background: var(--raised);
  }
  .checker {
    background-color: var(--check-a);
    background-image: conic-gradient(var(--check-b) 25%, transparent 0 50%, var(--check-b) 0 75%, transparent 0);
    background-size: 16px 16px;
  }
  .preview img,
  .preview video {
    display: block;
    max-width: 100%;
    max-height: 340px;
    object-fit: contain;
  }
  .preview video {
    width: 100%;
    background: #000;
  }
  .contain img {
    min-width: 128px;
    min-height: 128px;
    image-rendering: auto;
  }
  .glyph {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    padding: 32px 16px;
    color: var(--muted);
    font-size: var(--t-sm);
  }
  .glyph audio {
    width: 300px;
    max-width: 100%;
  }

  .name {
    margin: 18px 0 14px;
    font-size: var(--t-xl);
    font-weight: 650;
    line-height: 1.15;
    letter-spacing: -0.01em;
    font-variation-settings: 'opsz' 32;
    overflow-wrap: anywhere;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .meta {
    display: grid;
    grid-template-columns: 76px 1fr;
    gap: 10px 12px;
    margin: 24px 0 0;
    padding-top: 18px;
    border-top: 1px solid var(--line);
    font-size: var(--t-sm);
  }
  dt {
    color: var(--muted);
  }
  dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .label-input {
    display: block;
    width: 100%;
    margin-top: 6px;
    height: 30px;
  }
  .label-input::placeholder {
    color: var(--muted);
  }
  .inline {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 10px;
  }
  .mini {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    margin: 4px 0 0 -6px;
    padding: 3px 6px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: var(--muted);
    font-size: var(--t-xs);
    font-weight: 550;
    cursor: pointer;
  }
  .inline .mini {
    margin: 0;
  }
  .mini:hover {
    color: var(--text);
    background: var(--raised);
  }
  .link {
    color: var(--muted);
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
    user-select: all;
  }
</style>
