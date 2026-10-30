<script lang="ts">
  import { untrack } from 'svelte';
  import * as api from '../lib/api';
  import { fmtDate, formatOf } from '../lib/format';
  import { app } from '../lib/store.svelte';
  import type { Entry } from '../lib/types';
  import Icon from './Icon.svelte';

  interface Props {
    entry: Entry;
    index: number;
    x: number;
    y: number;
    size: number;
    selected: boolean;
    focused: boolean;
    selecting: boolean;
    /** Shown in the detail drawer. */
    open: boolean;
    onclick: (e: MouseEvent, index: number) => void;
    ondblclick: (index: number) => void;
    onkeydown: (e: KeyboardEvent, index: number) => void;
  }

  let { entry, index, x, y, size, selected, focused, selecting, open, onclick, ondblclick, onkeydown }: Props = $props();

  // The "develop" moment: a tile that mounts right after its batch arrived fades in from
  // a safelight-amber wash. Tiles scrolled into view later just appear.
  const DEVELOP_MS = 520;
  const developDelay = untrack(() => {
    const at = app.arrival.get(entry.id);
    const age = at === undefined ? Infinity : performance.now() - at;
    return age < DEVELOP_MS ? -Math.round(age) : null;
  });

  let developing = $state(developDelay !== null);

  const fmt = $derived(formatOf(entry));
  const src = $derived(api.mediaUrl(entry));
  const poster = $derived(fmt === 'video' ? api.posterUrl(entry) : null);
  const date = $derived(fmtDate(entry.postedAt ?? entry.cachedAt));
  /** Small transparent art reads better whole than cropped. */
  const contain = $derived(entry.kind === 'emoji' || entry.kind === 'sticker' || entry.kind === 'interface');
  /** GIF services often serve MP4; those should loop like GIFs. */
  const loops = $derived(entry.kind === 'gif');

  let failed = $state(false);
  let video: HTMLVideoElement | undefined = $state();

  function preview(on: boolean) {
    if (!video || loops) return;
    if (on) {
      video.play().catch(() => {});
    } else {
      video.pause();
      video.currentTime = 0;
    }
  }

  // Deterministic "waveform" for audio tiles.
  const bars = $derived.by(() => {
    let s = entry.id * 2654435761;
    return Array.from({ length: 21 }, () => {
      s = (s * 1103515245 + 12345) & 0x7fffffff;
      return 0.25 + (s % 1000) / 1333;
    });
  });
</script>

<div
  class="tile"
  class:selected
  class:selecting
  class:open
  class:developing
  role="option"
  aria-selected={selected}
  aria-label={entry.fileName}
  tabindex={focused ? 0 : -1}
  data-index={index}
  style:transform="translate3d({x}px, {y}px, 0)"
  style:width="{size}px"
  style:height="{size}px"
  style:--develop-delay={developDelay !== null ? `${developDelay}ms` : undefined}
  onclick={(e) => onclick(e, index)}
  ondblclick={() => ondblclick(index)}
  onkeydown={(e) => onkeydown(e, index)}
  onpointerenter={() => preview(true)}
  onpointerleave={() => preview(false)}
  onfocus={() => preview(true)}
  onblur={() => preview(false)}>
  <div class="media" class:checker={fmt !== 'audio' && fmt !== 'other'} class:contain>
    {#if fmt === 'audio'}
      <div class="glyph">
        <svg viewBox="0 0 84 32" class="wave" aria-hidden="true">
          {#each bars as h, i (i)}
            <rect x={i * 4} y={16 - h * 15} width="2.2" height={h * 30} rx="1.1" />
          {/each}
        </svg>
        <span class="ext">{entry.ext}</span>
      </div>
    {:else if fmt === 'video'}
      {#if poster}
        <img class="fill" src={poster} alt="" loading="lazy" decoding="async" draggable="false" />
      {/if}
      {#if src}
        <!-- svelte-ignore a11y_media_has_caption -->
        <video
          bind:this={video}
          class="fill"
          {src}
          muted
          loop
          playsinline
          autoplay={loops}
          preload={loops ? 'auto' : 'metadata'}
          disablepictureinpicture></video>
      {:else if !poster}
        <div class="glyph"><Icon name="video" size={28} /><span class="ext">{entry.ext}</span></div>
      {/if}
      {#if !loops}
        <span class="badge" aria-hidden="true"><Icon name="play" size={11} /></span>
      {/if}
    {:else if fmt === 'other' || failed || !src}
      <div class="glyph"><Icon name="unknown" size={28} /><span class="ext">{entry.ext}</span></div>
    {:else}
      <img class="fill" {src} alt="" loading="lazy" decoding="async" draggable="false" onerror={() => (failed = true)} />
    {/if}
  </div>
  {#if developing}
    <div class="wash" aria-hidden="true" onanimationend={() => (developing = false)}></div>
  {/if}
  <div class="caption" aria-hidden="true">
    <span class="name">{entry.fileName}</span>
    {#if date}<span class="date num">{date}</span>{/if}
  </div>
  <span class="check" aria-hidden="true"><Icon name="check" size={13} /></span>
</div>

<style>
  .tile {
    position: absolute;
    top: 0;
    left: 0;
    border-radius: var(--r-tile);
    background: var(--raised);
    overflow: hidden;
    cursor: pointer;
    contain: layout style size;
    outline-offset: 2px;
  }
  .tile.open {
    outline: 2px solid var(--muted);
  }
  .tile:focus-visible {
    outline: 2px solid var(--accent);
  }

  .media {
    position: absolute;
    inset: 0;
    border-radius: var(--r-tile);
    overflow: hidden;
    transition: inset 120ms var(--ease);
  }
  .checker {
    background-color: var(--check-a);
    background-image: conic-gradient(var(--check-b) 25%, transparent 0 50%, var(--check-b) 0 75%, transparent 0);
    background-size: 16px 16px;
  }
  .fill {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .contain .fill {
    object-fit: contain;
    padding: 16%;
  }

  .glyph {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--muted);
  }
  .wave {
    width: 58%;
    fill: currentColor;
  }
  .ext {
    font-size: var(--t-xs);
    font-weight: 600;
    letter-spacing: 0.02em;
  }

  .badge {
    position: absolute;
    right: 6px;
    top: 6px;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: rgb(10 12 18 / 0.6);
    color: #fff;
    padding-left: 1px;
  }

  /* Hover caption: always light text on a dark gradient, whatever the theme. */
  .caption {
    position: absolute;
    inset: auto 0 0 0;
    padding: 22px 8px 7px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    background: linear-gradient(to top, rgb(8 10 16 / 0.82), rgb(8 10 16 / 0));
    color: #fff;
    opacity: 0;
    transition: opacity 120ms;
    pointer-events: none;
  }
  .name {
    font-size: var(--t-xs);
    font-weight: 550;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .date {
    font-size: 11px;
    color: rgb(255 255 255 / 0.78);
  }
  .tile:hover .caption,
  .tile:focus-visible .caption {
    opacity: 1;
  }

  .check {
    position: absolute;
    left: 7px;
    top: 7px;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    display: grid;
    place-items: center;
    border: 1.5px solid rgb(255 255 255 / 0.9);
    background: rgb(10 12 18 / 0.35);
    color: transparent;
    opacity: 0;
    transition:
      opacity 120ms,
      background-color 120ms;
  }
  .tile:hover .check,
  .tile:focus-visible .check,
  .selecting .check {
    opacity: 1;
  }
  .check:hover {
    background: rgb(10 12 18 / 0.6);
  }

  .selected {
    background: var(--accent-soft);
    box-shadow: inset 0 0 0 2px var(--accent);
  }
  .selected .media {
    inset: 9px;
    border-radius: 4px;
  }
  .selected .check {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-ink);
  }

  /* Develop */
  .developing .media {
    animation: develop 520ms var(--ease) var(--develop-delay, 0ms) both;
  }
  .wash {
    position: absolute;
    inset: 0;
    background: var(--accent);
    pointer-events: none;
    animation: wash 520ms ease-out var(--develop-delay, 0ms) both;
  }
  @keyframes develop {
    from {
      opacity: 0.35;
      filter: sepia(0.9) contrast(0.55) brightness(1.25);
    }
    to {
      opacity: 1;
      filter: none;
    }
  }
  @keyframes wash {
    from {
      opacity: 0.6;
    }
    to {
      opacity: 0;
    }
  }
</style>
