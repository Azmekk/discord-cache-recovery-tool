// Fake backend for browser development and screenshots. Never touches the file system
// or the network: everything, including the images, is generated here.
//
// Scenarios via query string: ?mock=none (no Discord install), ?mock=empty (cache is empty),
// ?mock=big (20,000 entries, for performance work). Add &slow=4 to stretch the scan.

import type { Backend } from './api';
import type { Entry, ScanEvent, Source } from './types';

type Rng = () => number;

function mulberry32(seed: number): Rng {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const pick = <T>(r: Rng, xs: readonly T[]): T => xs[Math.floor(r() * xs.length)];
const int = (r: Rng, lo: number, hi: number) => Math.floor(lo + r() * (hi - lo + 1));
const hex = (r: Rng, n: number) => Array.from({ length: n }, () => Math.floor(r() * 16).toString(16)).join('');
const B62 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
const b62 = (r: Rng, n: number) => Array.from({ length: n }, () => B62[Math.floor(r() * 62)]).join('');

const DISCORD_EPOCH = BigInt(1420070400000); // no bigint literals: older build targets
function snowflake(ms: number, r: Rng): string {
  return (((BigInt(Math.floor(ms)) - DISCORD_EPOCH) << BigInt(22)) | BigInt(Math.floor(r() * 4194304))).toString();
}

const DAY = 86_400_000;
const YEAR = 365 * DAY;

const PHOTO_NAMES = ['IMG_', 'PXL_', 'DSC_', 'image', 'Screenshot_', 'unknown', 'photo_'];
const NAMED_IMAGES = [
  'setup-final.png', 'my-cat-loaf.jpg', 'meme.png', 'floorplan-v2.png', 'sunset_over_lake.jpg', 'ticket.png',
  'fit-check.jpg', 'boss-fight.png', 'patch-notes.png', 'concert.jpg', 'dinner.jpeg', 'sketch-wip.png',
  'reference-sheet.png', 'receipt.jpg', 'new-keyboard.jpg', 'map-route.png', 'birthday-cake.jpg', 'wallpaper.png',
];
const GIF_NAMES = [
  'cat-dancing', 'happy-dog', 'vibing', 'excited-yes', 'mind-blown', 'thumbs-up', 'facepalm', 'party-parrot',
  'confused-travolta', 'shrug', 'clapping', 'this-is-fine', 'sad-hamster', 'spongebob-imagination', 'nodding',
  'office-no', 'bongo-cat', 'dramatic-chipmunk', 'wave-hello', 'hug',
];
const VIDEO_NAMES = ['clip.mp4', 'replay.mp4', 'funny.mov', 'IMG_4471.MOV', 'recording.webm', 'highlight.mp4', 'trailer.mp4'];
const AUDIO_NAMES = ['voice-message.ogg', 'demo-track.mp3', 'meeting-notes.m4a', 'loop.wav'];
const EMBED_HOSTS = ['i.imgur.com', 'pbs.twimg.com', 'i.redd.it', 'upload.wikimedia.org', 'cdn.bsky.app', 'i.ytimg.com'];

type Look = 'photo' | 'screen' | 'gif' | 'emoji' | 'sticker' | 'avatar' | 'server' | 'ui' | 'video' | 'audio' | 'blob';

interface Spec {
  look: Look;
  w: number;
  h: number;
  hue: number;
  seed: number;
}

const specs = new Map<number, Spec>();
const svgCache = new Map<number, string>();

const MIME: Record<string, string> = {
  png: 'image/png', jpg: 'image/jpeg', jpeg: 'image/jpeg', webp: 'image/webp', gif: 'image/gif', svg: 'image/svg+xml',
  mp4: 'video/mp4', mov: 'video/quicktime', webm: 'video/webm', ogg: 'audio/ogg', mp3: 'audio/mpeg', m4a: 'audio/mp4',
  wav: 'audio/wav',
};
const mimeFor = (ext: string) => MIME[ext.toLowerCase()] ?? 'application/octet-stream';
const extOf = (name: string) => name.slice(name.lastIndexOf('.') + 1);

type Draft = Omit<Entry, 'id' | 'mime' | 'ext' | 'source'>;

function generate(count: number, sources: Source[], idBase: number, seed: number): Entry[] {
  const r = mulberry32(seed);
  const now = Date.now();
  const channels = Array.from({ length: 16 }, () => snowflake(now - YEAR * (2 + r() * 4), r));
  const guilds = Array.from({ length: 7 }, () => snowflake(now - YEAR * (3 + r() * 4), r));
  const out: Entry[] = [];
  let id = idBase;

  const recent = () => now - Math.pow(r(), 1.7) * 2 * YEAR;
  const cachedAfter = (t: number) => Math.min(now - 60_000, t + r() * r() * 20 * DAY);
  const src = () => (sources.length > 1 && r() < 0.18 ? sources[1].label : (sources[0]?.label ?? 'Discord'));

  const push = (e: Draft, look: Look, w: number, h: number, hue: number) => {
    const ext = extOf(e.fileName);
    out.push({ ...e, id, ext, mime: mimeFor(ext), source: src() });
    specs.set(id, { look, w, h, hue, seed: Math.floor(r() * 1e9) });
    id++;
  };

  while (out.length < count) {
    const roll = r();
    const hue = Math.floor(r() * 360);
    if (roll < 0.45) {
      // Attachment
      const postedAt = recent();
      const ch = pick(r, channels);
      const att = snowflake(postedAt, r);
      const t = r();
      let fileName: string;
      let look: Look;
      let size: number;
      if (t < 0.62) {
        const base = pick(r, PHOTO_NAMES);
        if (r() < 0.35) fileName = pick(r, NAMED_IMAGES);
        else if (base === 'Screenshot_')
          fileName = `Screenshot_${new Date(postedAt).toISOString().slice(0, 10)}_${int(r, 100000, 235959)}.png`;
        else fileName = `${base}${base.endsWith('_') ? int(r, 1000, 9999) : ''}.${pick(r, ['png', 'jpg', 'jpg', 'webp'])}`;
        look = /screen|patch|floorplan|map|ticket|receipt/i.test(fileName) ? 'screen' : 'photo';
        size = int(r, 40_000, 6_500_000);
      } else if (t < 0.78) {
        fileName = `${pick(r, GIF_NAMES)}.gif`;
        look = 'gif';
        size = int(r, 200_000, 8_000_000);
      } else if (t < 0.92) {
        fileName = pick(r, VIDEO_NAMES);
        look = 'video';
        size = int(r, 900_000, 45_000_000);
      } else {
        fileName = pick(r, AUDIO_NAMES);
        look = 'audio';
        size = int(r, 30_000, 2_400_000);
      }
      const [aw, ah] = pick(r, [[4, 3], [3, 4], [16, 9], [1, 1], [9, 16], [3, 2]] as const);
      const enc = encodeURIComponent(fileName);
      const base: Draft = {
        url: `https://cdn.discordapp.com/attachments/${ch}/${att}/${enc}`,
        originalUrl: null, host: 'cdn.discordapp.com', kind: 'attachment', fileName, size,
        postedAt, cachedAt: cachedAfter(postedAt), channelId: ch, guildId: null, groupKey: `att:${att}`,
      };
      push(base, look, aw * 200, ah * 200, hue);
      // Discord also caches resized previews of the same image.
      if ((look === 'photo' || look === 'screen') && r() < 0.3) {
        push(
          {
            ...base,
            url: `https://media.discordapp.net/attachments/${ch}/${att}/${enc}?width=400&height=300`,
            host: 'media.discordapp.net',
            size: Math.floor(size * (0.05 + r() * 0.1)),
            fileName: fileName.replace(/\.\w+$/, '.webp'),
            cachedAt: cachedAfter(postedAt),
          },
          look, aw * 200, ah * 200, hue,
        );
      }
    } else if (roll < 0.6) {
      const name = pick(r, GIF_NAMES);
      const asVideo = r() < 0.4;
      const service = pick(r, ['tenor', 'tenor', 'tenor', 'giphy', 'klipy']);
      const host = service === 'tenor' ? 'media.tenor.com' : service === 'giphy' ? 'media.giphy.com' : 'static.klipy.com';
      const fileName = `${name}.${asVideo ? 'mp4' : 'gif'}`;
      const key = b62(r, 16);
      push(
        { url: `https://${host}/${key}/${fileName}`, originalUrl: null, host, kind: 'gif', fileName, size: int(r, 120_000, 5_000_000), postedAt: null, cachedAt: recent(), channelId: null, guildId: null, groupKey: `gif:${key}` },
        asVideo ? 'video' : 'gif', pick(r, [480, 498, 640]), pick(r, [270, 360, 480, 498]), hue,
      );
    } else if (roll < 0.72) {
      const eid = snowflake(now - r() * 4 * YEAR, r);
      const animated = r() < 0.2;
      const fileName = `${eid}.${animated ? 'gif' : 'webp'}`;
      const e: Draft = { url: `https://cdn.discordapp.com/emojis/${fileName}?size=48`, originalUrl: null, host: 'cdn.discordapp.com', kind: 'emoji', fileName, size: int(r, 900, 60_000), postedAt: null, cachedAt: recent(), channelId: null, guildId: null, groupKey: `emoji:${eid}` };
      push(e, animated ? 'gif' : 'emoji', 96, 96, hue);
      if (r() < 0.4) push({ ...e, url: e.url!.replace('size=48', 'size=96'), size: Math.floor(e.size * 2.6) }, animated ? 'gif' : 'emoji', 96, 96, hue);
    } else if (roll < 0.76) {
      const sid = snowflake(now - r() * 3 * YEAR, r);
      push(
        { url: `https://media.discordapp.net/stickers/${sid}.webp?size=160`, originalUrl: null, host: 'media.discordapp.net', kind: 'sticker', fileName: `${sid}.webp`, size: int(r, 8_000, 120_000), postedAt: null, cachedAt: recent(), channelId: null, guildId: null, groupKey: `sticker:${sid}` },
        'sticker', 160, 160, hue,
      );
    } else if (roll < 0.86) {
      const uid = snowflake(now - r() * 7 * YEAR, r);
      const h = hex(r, 32);
      push(
        { url: `https://cdn.discordapp.com/avatars/${uid}/${h}.webp?size=80`, originalUrl: null, host: 'cdn.discordapp.com', kind: 'avatar', fileName: `${h}.webp`, size: int(r, 2_500, 40_000), postedAt: null, cachedAt: recent(), channelId: null, guildId: null, groupKey: `avatar:${uid}:${h}` },
        'avatar', 128, 128, hue,
      );
    } else if (roll < 0.9) {
      const g = pick(r, guilds);
      const h = hex(r, 32);
      const what = pick(r, ['icons', 'icons', 'banners', 'splashes']);
      const icon = what === 'icons';
      push(
        { url: `https://cdn.discordapp.com/${what}/${g}/${h}.png?size=${icon ? 96 : 480}`, originalUrl: null, host: 'cdn.discordapp.com', kind: 'server', fileName: `${h}.png`, size: int(r, 5_000, 900_000), postedAt: null, cachedAt: recent(), channelId: null, guildId: g, groupKey: `server:${h}` },
        icon ? 'server' : 'photo', icon ? 128 : 960, icon ? 128 : 540, hue,
      );
    } else if (roll < 0.97) {
      const host = pick(r, EMBED_HOSTS);
      const name = `${b62(r, 7)}.${pick(r, ['jpg', 'png', 'jpg'])}`;
      const original = `https://${host}/${name}`;
      push(
        { url: `https://images-ext-1.discordapp.net/external/${b62(r, 43)}/https/${host}/${name}`, originalUrl: original, host: 'images-ext-1.discordapp.net', kind: 'embed', fileName: name, size: int(r, 30_000, 1_800_000), postedAt: null, cachedAt: recent(), channelId: null, guildId: null, groupKey: `embed:${original}` },
        'photo', pick(r, [1200, 800, 640]), pick(r, [630, 800, 480]), hue,
      );
    } else if (roll < 0.99) {
      const name = `${hex(r, 20)}.svg`;
      push(
        { url: `https://discord.com/assets/${name}`, originalUrl: null, host: 'discord.com', kind: 'interface', fileName: name, size: int(r, 400, 9_000), postedAt: null, cachedAt: recent(), channelId: null, guildId: null, groupKey: `ui:${name}` },
        'ui', 96, 96, 235,
      );
    } else {
      const sniffed = r() < 0.7;
      const fileName = `f_${hex(r, 6)}.${sniffed ? 'png' : 'bin'}`;
      push(
        { url: null, originalUrl: null, host: null, kind: 'unknown', fileName, size: int(r, 2_000, 2_000_000), postedAt: null, cachedAt: recent(), channelId: null, guildId: null, groupKey: `orphan:${fileName}` },
        sniffed ? 'photo' : 'blob', 640, 480, hue,
      );
    }
  }

  // Cache order is effectively random.
  for (let i = out.length - 1; i > 0; i--) {
    const j = Math.floor(r() * (i + 1));
    [out[i], out[j]] = [out[j], out[i]];
  }
  return out;
}

// ---------- Images ----------

function svgFor(id: number): string {
  const s = specs.get(id);
  if (!s) return '';
  const r = mulberry32(s.seed);
  const { w, h, hue } = s;
  const c = (dh: number, sat: number, light: number) => `hsl(${(hue + dh + 360) % 360} ${sat}% ${light}%)`;
  let body: string;
  switch (s.look) {
    case 'photo':
    case 'video': {
      const sunX = w * (0.2 + r() * 0.6);
      const sunY = h * (0.2 + r() * 0.3);
      const m1 = Array.from({ length: 6 }, (_, i) => `${(w / 5) * i},${h * (0.5 + r() * 0.2)}`).join(' ');
      const m2 = Array.from({ length: 5 }, (_, i) => `${(w / 4) * i},${h * (0.65 + r() * 0.15)}`).join(' ');
      body = `<defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${c(0, 55, 70)}"/><stop offset="1" stop-color="${c(40, 60, 45)}"/></linearGradient></defs>
<rect width="${w}" height="${h}" fill="url(#g)"/>
<circle cx="${sunX}" cy="${sunY}" r="${Math.min(w, h) * (0.08 + r() * 0.08)}" fill="${c(-30, 90, 85)}"/>
<polygon points="0,${h} ${m1} ${w},${h}" fill="${c(160, 30, 35)}"/>
<polygon points="0,${h} ${m2} ${w},${h}" fill="${c(170, 35, 22)}"/>`;
      if (s.look === 'video') body += `<rect width="${w}" height="${h}" fill="#000" opacity=".25"/>`;
      break;
    }
    case 'screen': {
      const dark = r() < 0.6;
      const fg = dark ? '#3a3e4c' : '#d8dbe3';
      let rows = '';
      for (let y = h * 0.16; y < h * 0.92; y += h * (0.06 + r() * 0.05)) {
        rows += `<rect x="${w * 0.24}" y="${y}" width="${w * (0.25 + r() * 0.6)}" height="${h * 0.022}" rx="4" fill="${fg}"/>`;
      }
      body = `<rect width="${w}" height="${h}" fill="${dark ? '#1e2028' : '#f4f5f8'}"/><rect width="${w * 0.2}" height="${h}" fill="${dark ? '#16181e' : '#e6e8ee'}"/>
<rect x="${w * 0.24}" y="${h * 0.05}" width="${w * 0.4}" height="${h * 0.05}" rx="6" fill="${c(0, 70, 60)}"/>${rows}`;
      break;
    }
    case 'gif': {
      const dur = 0.8 + r() * 1.2;
      const rad = Math.min(w, h) * 0.18;
      const sx = w * 0.15 + rad / 2;
      const sy = h * 0.15 + rad / 2;
      body = `<rect width="${w}" height="${h}" fill="${c(0, 60, 55)}"/>
<rect x="${w * 0.1}" y="${h * 0.62}" width="${w * 0.8}" height="${h * 0.06}" rx="${h * 0.03}" fill="${c(0, 50, 40)}"/>
<circle cx="${w / 2}" cy="${h * 0.4}" r="${rad}" fill="${c(180, 80, 75)}"><animate attributeName="cy" values="${h * 0.25};${h * 0.62 - rad};${h * 0.25}" dur="${dur.toFixed(2)}s" repeatCount="indefinite"/></circle>
<rect x="${w * 0.15}" y="${h * 0.15}" width="${rad}" height="${rad}" fill="${c(90, 70, 70)}"><animateTransform attributeName="transform" type="rotate" from="0 ${sx} ${sy}" to="360 ${sx} ${sy}" dur="${(dur * 2).toFixed(2)}s" repeatCount="indefinite"/></rect>`;
      break;
    }
    case 'emoji':
      body =
        r() < 0.6
          ? `<circle cx="48" cy="48" r="42" fill="hsl(${45 + int(r, -8, 8)} 95% 60%)"/><circle cx="34" cy="40" r="6" fill="#3b2a12"/><circle cx="62" cy="40" r="6" fill="#3b2a12"/><path d="M28 58 Q48 ${70 + int(r, -6, 8)} 68 58" stroke="#3b2a12" stroke-width="6" fill="none" stroke-linecap="round"/>`
          : `<path d="M48 84 L14 48 A20 20 0 0 1 48 20 A20 20 0 0 1 82 48 Z" fill="${c(0, 80, 60)}"/>`;
      break;
    case 'sticker':
      body = `<path d="M30 20 Q80 4 130 26 Q156 80 132 132 Q80 156 28 134 Q4 80 30 20Z" fill="${c(0, 70, 62)}" stroke="#fff" stroke-width="8"/>
<circle cx="62" cy="70" r="9" fill="#222"/><circle cx="100" cy="70" r="9" fill="#222"/><path d="M60 100 Q81 118 102 100" stroke="#222" stroke-width="7" fill="none" stroke-linecap="round"/>`;
      break;
    case 'avatar':
      body = `<rect width="128" height="128" fill="${c(0, 45, 50)}"/><circle cx="64" cy="52" r="24" fill="${c(0, 40, 82)}"/><path d="M20 128 Q64 70 108 128Z" fill="${c(0, 40, 82)}"/>`;
      break;
    case 'server': {
      const letters = String.fromCharCode(65 + int(r, 0, 25)) + String.fromCharCode(97 + int(r, 0, 25));
      body = `<rect width="128" height="128" rx="36" fill="${c(0, 55, 45)}"/><text x="64" y="80" font-family="sans-serif" font-size="46" font-weight="700" text-anchor="middle" fill="#fff">${letters}</text>`;
      break;
    }
    case 'ui':
      body = `<rect x="18" y="26" width="60" height="44" rx="8" fill="none" stroke="#8b94ab" stroke-width="6"/><path d="M30 60 l12-14 10 10 8-8 8 12" stroke="#8b94ab" stroke-width="5" fill="none"/>`;
      break;
    default:
      return '';
  }
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${body}</svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

function imageFor(entry: Entry): string {
  let u = svgCache.get(entry.id);
  if (u === undefined) {
    u = svgFor(entry.id);
    svgCache.set(entry.id, u);
  }
  return u;
}

// ---------- Backend ----------

const sleep = (ms: number) => new Promise((res) => setTimeout(res, ms));

export function createMockBackend(): Backend {
  const scenario = new URLSearchParams(location.search).get('mock') ?? 'default';
  // ?slow=4 stretches the scan for screenshots of the develop animation.
  const slow = Math.max(1, Number(new URLSearchParams(location.search).get('slow')) || 1);
  const detected: Source[] =
    scenario === 'none'
      ? []
      : [
          { label: 'Discord', path: 'C:\\Users\\you\\AppData\\Roaming\\discord\\Cache\\Cache_Data', format: 'blockfile' },
          { label: 'Discord PTB', path: 'C:\\Users\\you\\AppData\\Roaming\\discordptb\\Cache\\Cache_Data', format: 'blockfile' },
        ];
  const count = scenario === 'big' ? 20_000 : scenario === 'empty' || scenario === 'none' ? 0 : 3_000;
  let scanToken = 0;
  let nextId = 1;
  let detectedEntries: Entry[] | null = null;
  const folderEntries = new Map<string, Entry[]>();

  return {
    async listSources() {
      await sleep(120);
      return detected;
    },

    async scan(dirs, onEvent: (ev: ScanEvent) => void) {
      const token = ++scanToken;
      const sources: Source[] = dirs
        ? dirs.map((path) => ({ label: 'Chosen folder', path, format: 'blockfile' as const }))
        : detected;
      let entries: Entry[];
      if (dirs) {
        const key = dirs.join('|');
        entries = folderEntries.get(key) ?? generate(900, sources, (nextId += 1_000_000), 777);
        folderEntries.set(key, entries);
      } else {
        entries = detectedEntries ??= generate(count, sources, nextId, 20241);
      }
      const started = performance.now();
      onEvent({ type: 'started', data: { total: entries.length, sources } });
      const size = Math.max(1, Math.ceil(entries.length / 26));
      for (let i = 0; i < entries.length; i += size) {
        await sleep((70 + Math.random() * 30) * slow);
        if (token !== scanToken) return;
        const chunk = entries.slice(i, i + size);
        onEvent({ type: 'batch', data: { entries: chunk, done: i + chunk.length, total: entries.length } });
      }
      await sleep(entries.length ? 60 : 700);
      if (token !== scanToken) return;
      onEvent({
        type: 'finished',
        data: { count: entries.length, elapsedMs: Math.round(performance.now() - started), errors: entries.length ? 3 : 0 },
      });
    },

    async exportEntries(ids, dest) {
      await sleep(500 + Math.min(ids.length, 1000));
      return { written: ids.length, failed: 0, dest };
    },

    mediaUrl(entry) {
      // Generated SVGs can't stand in for real audio/video; components treat '' as "not playable".
      if (entry.mime.startsWith('video/') || entry.mime.startsWith('audio/')) return '';
      return imageFor(entry);
    },

    posterUrl(entry) {
      return entry.mime.startsWith('video/') ? imageFor(entry) : null;
    },

    async pickDirectory(title) {
      await sleep(200);
      return /cache/i.test(title)
        ? 'D:\\Backups\\Old PC\\discord\\Cache\\Cache_Data'
        : 'C:\\Users\\you\\Pictures\\Recovered from Discord';
    },

    async openUrl(url) {
      console.info('[mock] would open', url);
    },

    async revealInFolder(path) {
      console.info('[mock] would reveal', path);
    },
  };
}
