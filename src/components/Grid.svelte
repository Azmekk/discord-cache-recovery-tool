<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { app } from '../lib/store.svelte';
  import EmptyState from './EmptyState.svelte';
  import Tile from './Tile.svelte';

  let scroller: HTMLDivElement | undefined = $state();
  let width = $state(0);
  let height = $state(0);
  let scrollTop = $state(0);
  let focusIndex = $state(0);
  let anchor = -1;

  const PAD = 20;
  const BOTTOM = 96; // keeps the last row clear of the floating selection bar
  const OVERSCAN = 2;

  const gap = $derived(app.tileSize < 130 ? 6 : 8);
  const inner = $derived(Math.max(0, width - PAD * 2));
  const cols = $derived(Math.max(1, Math.floor((inner + gap) / (app.tileSize + gap))));
  const cell = $derived(Math.max(48, Math.floor((inner - gap * (cols - 1)) / cols)));
  const rowH = $derived(cell + gap);
  const count = $derived(app.filtered.length);
  const rows = $derived(Math.ceil(count / cols));
  const totalH = $derived(rows ? PAD + rows * rowH - gap + BOTTOM : 0);

  const visible = $derived.by(() => {
    if (!width || !count) return [];
    const first = Math.max(0, Math.floor((scrollTop - PAD) / rowH) - OVERSCAN);
    const last = Math.min(rows - 1, Math.floor((scrollTop - PAD + height) / rowH) + OVERSCAN);
    const start = first * cols;
    const end = Math.min(count, (last + 1) * cols);
    const list = app.filtered;
    const out = [];
    for (let i = start; i < end; i++) {
      out.push({ entry: list[i], i, x: PAD + (i % cols) * (cell + gap), y: PAD + Math.floor(i / cols) * rowH });
    }
    return out;
  });

  const selecting = $derived(app.selected.size > 0);
  // If the roving-focus tile is scrolled away, let the canvas itself take Tab focus.
  const focusRendered = $derived(visible.some((t) => t.i === focusIndex));

  function onCanvasFocus(e: FocusEvent) {
    if (e.target !== e.currentTarget) return;
    const first = visible.find((t) => t.y >= scrollTop) ?? visible[0];
    if (first) void moveFocus(first.i);
  }

  // New filters or sort: back to the top. Streaming batches don't change viewKey.
  $effect(() => {
    void app.viewKey;
    if (scroller) scroller.scrollTop = 0;
    scrollTop = 0;
    focusIndex = 0;
    anchor = -1;
  });

  $effect(() => {
    if (focusIndex > 0 && focusIndex >= count) focusIndex = Math.max(0, count - 1);
  });

  // When the layout reflows (window resize, drawer opening, tile size), keep the same
  // files at the top of the viewport instead of jumping somewhere else.
  let prev = { cols: 0, rowH: 0 };
  $effect.pre(() => {
    const c = cols;
    const r = rowH;
    untrack(() => {
      if (scroller && prev.cols && (prev.cols !== c || prev.rowH !== r) && scroller.scrollTop > 0) {
        const pos = Math.max(0, scroller.scrollTop - PAD) / prev.rowH;
        const topIndex = Math.floor(pos) * prev.cols;
        scroller.scrollTop = Math.floor(topIndex / c) * r + (pos % 1) * r;
        scrollTop = scroller.scrollTop;
      }
      prev = { cols: c, rowH: r };
    });
  });

  function ensureVisible(i: number) {
    if (!scroller) return;
    const y = PAD + Math.floor(i / cols) * rowH;
    if (y - PAD < scroller.scrollTop) scroller.scrollTop = Math.max(0, y - PAD);
    else if (y + cell + PAD > scroller.scrollTop + height) scroller.scrollTop = y + cell + PAD - height;
    scrollTop = scroller.scrollTop;
  }

  async function moveFocus(i: number) {
    focusIndex = i;
    if (app.openId != null) app.openId = app.filtered[i].id;
    ensureVisible(i);
    await tick();
    scroller?.querySelector<HTMLElement>(`[data-index="${i}"]`)?.focus({ preventScroll: true });
  }

  function selectRange(a: number, b: number, additive: boolean) {
    const [lo, hi] = a < b ? [a, b] : [b, a];
    const next = new Set(additive ? app.selected : []);
    for (let k = lo; k <= hi; k++) next.add(app.filtered[k].id);
    app.setSelected(next);
  }

  function open(i: number) {
    focusIndex = i;
    anchor = i;
    app.openId = app.filtered[i].id;
  }

  function onTileClick(e: MouseEvent, i: number) {
    const entry = app.filtered[i];
    const onCheck = (e.target as HTMLElement).closest('.check') !== null;
    focusIndex = i;
    if (e.shiftKey) {
      selectRange(anchor >= 0 ? anchor : i, i, e.ctrlKey || e.metaKey);
      return;
    }
    anchor = i;
    // Once anything is selected, a click keeps selecting (double-click still opens).
    if (e.ctrlKey || e.metaKey || onCheck || selecting) app.toggleSelected(entry.id);
    else app.openId = entry.id;
  }

  function onTileKey(e: KeyboardEvent, i: number) {
    const page = cols * Math.max(1, Math.floor(height / rowH));
    let next: number;
    switch (e.key) {
      case 'ArrowRight':
        next = i + 1;
        break;
      case 'ArrowLeft':
        next = i - 1;
        break;
      case 'ArrowDown':
        next = i + cols;
        break;
      case 'ArrowUp':
        next = i - cols;
        break;
      case 'PageDown':
        next = i + page;
        break;
      case 'PageUp':
        next = i - page;
        break;
      case 'Home':
        next = 0;
        break;
      case 'End':
        next = count - 1;
        break;
      case 'Enter':
      case ' ':
        e.preventDefault();
        if (e.ctrlKey || e.metaKey) {
          app.toggleSelected(app.filtered[i].id);
          anchor = i;
        } else open(i);
        return;
      default:
        return;
    }
    e.preventDefault();
    next = Math.max(0, Math.min(count - 1, next));
    if (e.shiftKey) {
      if (anchor < 0) anchor = i;
      selectRange(anchor, next, false);
    }
    void moveFocus(next);
  }
</script>

<div
  class="scroller"
  bind:this={scroller}
  bind:clientWidth={width}
  bind:clientHeight={height}
  onscroll={() => (scrollTop = scroller!.scrollTop)}>
  {#if count === 0}
    <EmptyState />
  {:else}
    <div
      class="canvas"
      role="listbox"
      aria-label="Recovered files"
      aria-multiselectable="true"
      tabindex={focusRendered ? -1 : 0}
      onfocus={onCanvasFocus}
      style:height="{totalH}px">
      {#each visible as t (t.entry.id)}
        <Tile
          entry={t.entry}
          index={t.i}
          x={t.x}
          y={t.y}
          size={cell}
          selected={app.selected.has(t.entry.id)}
          focused={t.i === focusIndex}
          {selecting}
          open={t.entry.id === app.openId}
          onclick={onTileClick}
          ondblclick={open}
          onkeydown={onTileKey} />
      {/each}
    </div>
  {/if}
</div>

<style>
  .scroller {
    position: absolute;
    inset: 0;
    overflow-y: auto;
    overflow-x: hidden;
    overscroll-behavior: contain;
  }
  .canvas {
    position: relative;
    user-select: none;
  }
  .canvas:focus {
    outline: none;
  }
</style>
