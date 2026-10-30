import type { Entry, Kind } from './types';

const num = new Intl.NumberFormat();
export const fmtCount = (n: number) => num.format(n);

export function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ['KB', 'MB', 'GB'];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

const shortDate = new Intl.DateTimeFormat(undefined, { day: 'numeric', month: 'short', year: 'numeric' });
const longDate = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' });

export const fmtDate = (ms: number | null) => (ms == null ? '' : shortDate.format(ms));
export const fmtDateTime = (ms: number | null) => (ms == null ? 'Unknown' : longDate.format(ms));

export const plural = (n: number, one: string, many = one + 's') => `${fmtCount(n)} ${n === 1 ? one : many}`;

export type FormatKey = 'image' | 'gif' | 'video' | 'audio';

export function formatOf(e: Entry): FormatKey | 'other' {
  const m = e.mime.toLowerCase();
  if (m === 'image/gif' || e.ext.toLowerCase() === 'gif') return 'gif';
  if (m.startsWith('image/')) return 'image';
  if (m.startsWith('video/')) return 'video';
  if (m.startsWith('audio/')) return 'audio';
  return 'other';
}

export const kindLabels: Record<Kind, string> = {
  attachment: 'Attachments',
  gif: 'GIFs',
  emoji: 'Emojis',
  sticker: 'Stickers',
  avatar: 'Avatars',
  server: 'Server art',
  embed: 'Embeds',
  unknown: 'Unknown',
  interface: 'Interface images',
};

export const kindSingular: Record<Kind, string> = {
  attachment: 'Attachment',
  gif: 'GIF',
  emoji: 'Emoji',
  sticker: 'Sticker',
  avatar: 'Avatar',
  server: 'Server art',
  embed: 'Embed',
  unknown: 'Unknown',
  interface: 'Discord interface image',
};
