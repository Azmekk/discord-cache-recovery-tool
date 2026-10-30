import * as api from './api';
import { formatOf, plural, type FormatKey } from './format';
import { loadPref, savePref } from './prefs';
import type { Entry, Kind, ScanEvent, Source } from './types';

export type DateRange = 'any' | 'today' | 'week' | 'month' | 'year' | 'custom';
export type SortKey = 'posted' | 'cached' | 'size' | 'name';
export type KindFilter = Kind | 'all';

export interface Toast {
  id: number;
  message: string;
  tone: 'info' | 'error';
  action?: { label: string; run: () => void };
}

interface ScanState {
  phase: 'idle' | 'scanning' | 'done' | 'error';
  done: number;
  total: number;
  errors: number;
  message: string;
  /** Sources Discord held locked during the last scan. */
  lockedSources: string[];
  diagnostics: string;
}

const emptyScan = (phase: ScanState['phase']): ScanState => ({
  phase,
  done: 0,
  total: 0,
  errors: 0,
  message: '',
  lockedSources: [],
  diagnostics: '',
});

const DAY = 86_400_000;
const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });
export const TILE_MIN = 96;
export const TILE_MAX = 260;

const timeOf = (e: Entry) => e.postedAt ?? e.cachedAt;

// Search haystacks are built once per entry (labels are appended at match time).
const haystacks = new WeakMap<Entry, string>();
function haystackOf(e: Entry): string {
  let h = haystacks.get(e);
  if (h === undefined) {
    h = [e.fileName, e.url, e.originalUrl, e.host, e.channelId].filter(Boolean).join('\n').toLowerCase();
    haystacks.set(e, h);
  }
  return h;
}

function startOfDay(ms: number) {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

function dateBounds(range: DateRange, from: string, to: string): [number, number] | null {
  const now = Date.now();
  switch (range) {
    case 'any':
      return null;
    case 'today':
      return [startOfDay(now), Infinity];
    case 'week':
      return [now - 7 * DAY, Infinity];
    case 'month':
      return [now - 30 * DAY, Infinity];
    case 'year':
      return [now - 365 * DAY, Infinity];
    case 'custom': {
      const lo = from ? new Date(`${from}T00:00:00`).getTime() : -Infinity;
      const hi = to ? new Date(`${to}T00:00:00`).getTime() + DAY : Infinity;
      return [Number.isNaN(lo) ? -Infinity : lo, Number.isNaN(hi) ? Infinity : hi];
    }
  }
}

class AppState {
  // ---- Data ----
  entries = $state.raw<Entry[]>([]);
  detected = $state.raw<Source[] | null>(null);
  sources = $state.raw<Source[]>([]);
  scan = $state<ScanState>(emptyScan('idle'));
  /** When each entry arrived (performance.now()), for the develop animation. Not reactive on purpose. */
  arrival = new Map<number, number>();

  // ---- Filters ----
  query = $state('');
  kind = $state<KindFilter>('all');
  showInterface = $state(false);
  formats = $state.raw<ReadonlySet<FormatKey>>(new Set());
  dateRange = $state<DateRange>('any');
  customFrom = $state('');
  customTo = $state('');
  sourceFilter = $state<string | null>(null);

  // ---- View ----
  sort = $state<SortKey>(loadPref<SortKey>('dcr.sort', 'posted'));
  tileSize = $state(Math.min(TILE_MAX, Math.max(TILE_MIN, loadPref('dcr.tileSize', 148))));
  groupSizes = $state(loadPref('dcr.groupSizes', true));
  selected = $state.raw<ReadonlySet<number>>(new Set());
  openId = $state<number | null>(null);
  channelLabels = $state<Record<string, string>>(loadPref('dcr.channelLabels', {}));
  toasts = $state<Toast[]>([]);

  // ---- Derived pipeline ----
  /** All entries, minus interface images (unless shown), with size variants collapsed. */
  visibleEntries = $derived.by(() => {
    const src = this.showInterface ? this.entries : this.entries.filter((e) => e.kind !== 'interface');
    if (!this.groupSizes) return src;
    const best = new Map<string, Entry>();
    for (const e of src) {
      const cur = best.get(e.groupKey);
      if (!cur || e.size > cur.size) best.set(e.groupKey, e);
    }
    return best.size === src.length ? src : Array.from(best.values());
  });

  /** Everything except the kind filter, so the kind list can show live counts. */
  base = $derived.by(() => {
    const tokens = this.query.toLowerCase().split(/\s+/).filter(Boolean);
    const formats = this.formats;
    const bounds = dateBounds(this.dateRange, this.customFrom, this.customTo);
    const source = this.sourceFilter;
    const labels = this.channelLabels;
    const hasLabels = Object.keys(labels).length > 0;
    if (!tokens.length && !formats.size && !bounds && !source) return this.visibleEntries;
    return this.visibleEntries.filter((e) => {
      if (source && e.source !== source) return false;
      if (formats.size) {
        const f = formatOf(e);
        if (f === 'other' || !formats.has(f)) return false;
      }
      if (bounds) {
        const t = timeOf(e);
        if (t == null || t < bounds[0] || t >= bounds[1]) return false;
      }
      if (tokens.length) {
        let hay = haystackOf(e);
        if (hasLabels && e.channelId && labels[e.channelId]) hay += '\n' + labels[e.channelId].toLowerCase();
        for (const t of tokens) if (!hay.includes(t)) return false;
      }
      return true;
    });
  });

  kindCounts = $derived.by(() => {
    const counts: Record<string, number> = {};
    for (const e of this.base) counts[e.kind] = (counts[e.kind] ?? 0) + 1;
    return counts as Partial<Record<Kind, number>>;
  });

  sourceCounts = $derived.by(() => {
    const counts: Record<string, number> = {};
    for (const e of this.visibleEntries) counts[e.source] = (counts[e.source] ?? 0) + 1;
    return counts;
  });

  filtered = $derived.by(() => {
    const k = this.kind;
    const list = k === 'all' ? this.base.slice() : this.base.filter((e) => e.kind === k);
    const byName = (a: Entry, b: Entry) => collator.compare(a.fileName, b.fileName);
    switch (this.sort) {
      case 'posted':
        list.sort((a, b) => (timeOf(b) ?? 0) - (timeOf(a) ?? 0) || b.id - a.id);
        break;
      case 'cached':
        list.sort((a, b) => (b.cachedAt ?? 0) - (a.cachedAt ?? 0) || b.id - a.id);
        break;
      case 'size':
        list.sort((a, b) => b.size - a.size || a.id - b.id);
        break;
      case 'name':
        list.sort((a, b) => byName(a, b) || a.id - b.id);
        break;
    }
    return list;
  });

  byId = $derived.by(() => {
    const m = new Map<number, Entry>();
    for (const e of this.entries) m.set(e.id, e);
    return m;
  });

  openEntry = $derived(this.openId == null ? null : (this.byId.get(this.openId) ?? null));

  filtersActive = $derived(
    this.query.trim() !== '' ||
      this.kind !== 'all' ||
      this.formats.size > 0 ||
      this.dateRange !== 'any' ||
      this.sourceFilter !== null,
  );

  /** Changes when the user changes what they're looking at (not when a batch arrives). */
  viewKey = $derived(
    [this.query, this.kind, [...this.formats].join(), this.dateRange, this.customFrom, this.customTo, this.sourceFilter, this.sort, this.groupSizes, this.showInterface].join('|'),
  );

  // ---- Actions ----
  private scanGen = 0;
  /** null = the detected Discord installs; otherwise folders the user chose. */
  lastDirs = $state.raw<string[] | null>(null);

  rescan() {
    return this.startScan(this.lastDirs);
  }

  async init() {
    try {
      this.detected = await api.listSources();
      this.sources = this.detected;
    } catch (err) {
      this.detected = [];
      this.toast(`Couldn't look for Discord installs: ${errorText(err)}`, 'error');
    }
    if (this.detected.length) void this.startScan(null);
    else this.scan = emptyScan('done');
  }

  async startScan(dirs: string[] | null) {
    const gen = ++this.scanGen;
    this.lastDirs = dirs;
    this.scan = emptyScan('scanning');
    this.entries = [];
    this.arrival.clear();
    this.selected = new Set();
    this.openId = null;
    this.sourceFilter = null;
    try {
      await api.scan(dirs, (ev) => {
        if (gen === this.scanGen) this.onScanEvent(ev);
      });
    } catch (err) {
      if (gen !== this.scanGen) return;
      this.scan = { ...this.scan, phase: 'error', message: errorText(err) };
      this.toast(`Scan stopped: ${errorText(err)}`, 'error');
    }
  }

  private onScanEvent(ev: ScanEvent) {
    switch (ev.type) {
      case 'started':
        this.sources = ev.data.sources;
        this.scan = { ...this.scan, total: ev.data.total };
        break;
      case 'batch': {
        const now = performance.now();
        for (const e of ev.data.entries) this.arrival.set(e.id, now);
        this.entries = this.entries.concat(ev.data.entries);
        this.scan = { ...this.scan, done: ev.data.done, total: ev.data.total };
        break;
      }
      case 'finished':
        this.scan = {
          ...this.scan,
          phase: 'done',
          done: this.scan.total,
          errors: ev.data.errors,
          lockedSources: ev.data.lockedSources,
          diagnostics: ev.data.diagnostics,
        };
        break;
    }
  }

  async copyDiagnostics() {
    try {
      await navigator.clipboard.writeText(this.scan.diagnostics);
      this.toast('Scan details copied');
    } catch (err) {
      this.toast(`Couldn't copy the scan details: ${errorText(err)}`, 'error');
    }
  }

  async chooseCacheFolder() {
    let dir: string | null;
    try {
      dir = await api.pickDirectory('Choose a Discord cache folder');
    } catch (err) {
      this.toast(`Couldn't open the folder picker: ${errorText(err)}`, 'error');
      return;
    }
    if (dir) void this.startScan([dir]);
  }

  async exportIds(ids: number[]) {
    if (!ids.length) return;
    let dest: string | null;
    try {
      dest = await api.pickDirectory(`Choose where to save ${plural(ids.length, 'file')}`);
    } catch (err) {
      this.toast(`Couldn't open the folder picker: ${errorText(err)}`, 'error');
      return;
    }
    if (!dest) return;
    try {
      const res = await api.exportEntries(ids, dest);
      const reveal = { label: 'Show folder', run: () => void api.revealInFolder(res.dest).catch(() => {}) };
      if (res.written === 0) {
        this.toast(`Nothing was exported. ${plural(res.failed, 'file')} couldn't be read from the cache.`, 'error');
      } else if (res.failed > 0) {
        this.toast(`Exported ${plural(res.written, 'file')}. ${plural(res.failed, 'file')} couldn't be read from the cache.`, 'error', reveal);
      } else {
        this.toast(`Exported ${plural(res.written, 'file')}`, 'info', reveal);
      }
    } catch (err) {
      this.toast(`Export failed: ${errorText(err)}`, 'error');
    }
  }

  // Selection
  setSelected(next: ReadonlySet<number>) {
    this.selected = next;
  }
  toggleSelected(id: number) {
    const next = new Set(this.selected);
    if (!next.delete(id)) next.add(id);
    this.selected = next;
  }
  selectAllFiltered() {
    this.selected = new Set(this.filtered.map((e) => e.id));
  }
  clearSelection() {
    if (this.selected.size) this.selected = new Set();
  }

  clearFilters() {
    this.query = '';
    this.kind = 'all';
    this.formats = new Set();
    this.dateRange = 'any';
    this.customFrom = '';
    this.customTo = '';
    this.sourceFilter = null;
  }

  toggleFormat(f: FormatKey) {
    const next = new Set(this.formats);
    if (!next.delete(f)) next.add(f);
    this.formats = next;
  }

  setInterfaceShown(v: boolean) {
    this.showInterface = v;
    if (!v && this.kind === 'interface') this.kind = 'all';
  }

  setTileSize(v: number) {
    this.tileSize = v;
    savePref('dcr.tileSize', v);
  }
  setSort(v: SortKey) {
    this.sort = v;
    savePref('dcr.sort', v);
  }
  setGroupSizes(v: boolean) {
    this.groupSizes = v;
    savePref('dcr.groupSizes', v);
  }

  setChannelLabel(channelId: string, label: string) {
    const next = { ...this.channelLabels };
    const clean = label.trim();
    if (clean) next[channelId] = clean;
    else delete next[channelId];
    this.channelLabels = next;
    savePref('dcr.channelLabels', next);
  }

  // Toasts
  private toastId = 0;
  toast(message: string, tone: Toast['tone'] = 'info', action?: Toast['action']) {
    const id = ++this.toastId;
    this.toasts = [...this.toasts.slice(-2), { id, message, tone, action }];
    setTimeout(() => this.dismissToast(id), tone === 'error' ? 10_000 : 6_000);
  }
  dismissToast(id: number) {
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }
}

export function errorText(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === 'string') return err;
  try {
    return JSON.stringify(err);
  } catch {
    return String(err);
  }
}

export const app = new AppState();
