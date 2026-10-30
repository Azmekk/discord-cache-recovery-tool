// Mirrors the IPC contract in docs/ARCHITECTURE.md. Do not change shapes here
// without changing the Rust side.

export type Kind =
  | 'attachment'
  | 'gif'
  | 'emoji'
  | 'sticker'
  | 'avatar'
  | 'server'
  | 'embed'
  | 'interface'
  | 'unknown';

export interface Entry {
  id: number;
  url: string | null;
  originalUrl: string | null;
  host: string | null;
  kind: Kind;
  fileName: string;
  mime: string;
  ext: string;
  size: number;
  postedAt: number | null;
  cachedAt: number | null;
  channelId: string | null;
  guildId: string | null;
  groupKey: string;
  source: string;
}

export interface Source {
  label: string;
  path: string;
  format: 'blockfile' | 'simple' | 'unknown';
}

export type ScanEvent =
  | { type: 'started'; data: { total: number; sources: Source[] } }
  | { type: 'batch'; data: { entries: Entry[]; done: number; total: number } }
  | { type: 'finished'; data: { count: number; elapsedMs: number; errors: number } };

export interface ExportResult {
  written: number;
  failed: number;
  dest: string;
}
