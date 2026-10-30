// The only module that talks to the backend. Outside Tauri (plain `npm run dev`)
// it swaps in ./mock.ts so the UI can be developed and screenshotted in a browser.

import type { Entry, ExportResult, ScanEvent, Source } from './types';

export interface Backend {
  listSources(): Promise<Source[]>;
  scan(dirs: string[] | null, onEvent: (ev: ScanEvent) => void): Promise<void>;
  exportEntries(ids: number[], dest: string): Promise<ExportResult>;
  mediaUrl(entry: Entry): string;
  /** Still image to show under a video tile. Real backend: none (the video's first frame is used). */
  posterUrl(entry: Entry): string | null;
  pickDirectory(title: string): Promise<string | null>;
  openUrl(url: string): Promise<void>;
  revealInFolder(path: string): Promise<void>;
}

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function createTauriBackend(): Promise<Backend> {
  const [{ invoke, Channel, convertFileSrc }, dialog, opener] = await Promise.all([
    import('@tauri-apps/api/core'),
    import('@tauri-apps/plugin-dialog'),
    import('@tauri-apps/plugin-opener'),
  ]);
  return {
    listSources: () => invoke<Source[]>('list_sources'),
    async scan(dirs, onEvent) {
      const channel = new Channel<ScanEvent>();
      channel.onmessage = onEvent;
      await invoke('scan', { dirs, onEvent: channel });
    },
    exportEntries: (ids, dest) => invoke<ExportResult>('export_entries', { ids, dest }),
    mediaUrl: (entry) => convertFileSrc(String(entry.id), 'media'),
    posterUrl: () => null,
    async pickDirectory(title) {
      const picked = await dialog.open({ directory: true, multiple: false, title });
      return typeof picked === 'string' ? picked : null;
    },
    openUrl: (url) => opener.openUrl(url),
    revealInFolder: (path) => opener.revealItemInDir(path),
  };
}

let backend: Backend | null = null;

export async function initBackend(): Promise<Backend> {
  if (backend) return backend;
  backend = isTauri ? await createTauriBackend() : (await import('./mock')).createMockBackend();
  return backend;
}

function b(): Backend {
  if (!backend) throw new Error('initBackend() must run before using the API');
  return backend;
}

export const listSources = () => b().listSources();
export const scan = (dirs: string[] | null, onEvent: (ev: ScanEvent) => void) => b().scan(dirs, onEvent);
export const exportEntries = (ids: number[], dest: string) => b().exportEntries(ids, dest);
export const mediaUrl = (entry: Entry) => b().mediaUrl(entry);
export const posterUrl = (entry: Entry) => b().posterUrl(entry);
export const pickDirectory = (title: string) => b().pickDirectory(title);
export const openUrl = (url: string) => b().openUrl(url);
export const revealInFolder = (path: string) => b().revealInFolder(path);
