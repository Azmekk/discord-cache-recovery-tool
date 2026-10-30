# Architecture

Tauri 2 app. Rust backend in `src-tauri/`, Svelte 5 + TypeScript frontend in `src/`.

The app is **offline and read-only**: it never makes network requests and never writes to
Discord's folders. It only reads Discord's local HTTP cache and copies files out when the
user exports them.

## Discord cache formats

Discord is Electron, so its cache is Chromium's HTTP disk cache, at
`<config>/discord*/Cache/Cache_Data`. Two on-disk formats exist:

### Blockfile (Windows)

`index`, `data_0` … `data_3` (+ overflow `data_4`…), and `f_XXXXXX` files.

- `data_N` files: 8192-byte `BlockFileHeader` (magic `0xC104CAC3`, `entry_size` at offset 12,
  allocation bitmap `u32[2028]` at offset 80), then fixed-size blocks.
  `data_0` = 36 B rankings, `data_1` = 256 B, `data_2` = 1 KiB, `data_3` = 4 KiB.
- `EntryStore` (256 B, lives in 256-byte block files): `hash@0, next@4, rankings@8,
  reuse@12, refetch@16, state@20, creation_time:u64@24, key_len@32, long_key:addr@36,
  data_size[4]@40, data_addr[4]@56, flags@72, self_hash@92, key[160]@96`. The inline key
  may continue into the following contiguous blocks; if `long_key` is set, the key lives there.
- Stream 0 = pickled `HttpResponseInfo` (response headers + times), stream 1 = body.
- `CacheAddr` (u32): bit 31 initialized, bits 28–30 file type (0 = external `f_%06x`,
  1 = 36 B, 2 = 256 B, 3 = 1 KiB, 4 = 4 KiB). External: bits 0–27 file number.
  Block: bits 16–23 file selector (`data_<sel>`), bits 24–25 block count − 1,
  bits 0–15 start block; offset = 8192 + start × block size.
- Small bodies (< ~16 KiB) live **inside** `data_N` blocks, not in `f_` files.
- While Discord runs it holds `index` and `data_N` open exclusively (Windows sharing
  violation), so names, links and dates can only be read with Discord fully closed. `f_` files
  stay readable. The scan reports this via `lockedSources` instead of failing silently.
- `creation_time` and the pickle times are `base::Time`: microseconds since 1601-01-01 UTC.

### Simple cache (Linux, macOS)

One file per entry, `<16 hex>_0`:

```
[SimpleFileHeader 24 B: magic u64 0xfcfb6d1ba7725c30, version u32, key_length u32, key_hash u32]
[key: key_length bytes]
[stream 1 = body]
[SimpleFileEOF 24 B: magic u64 0xf4fa6f45970d41d8, flags u32, crc32 u32, stream_size u32]
[stream 0 = HttpResponseInfo pickle]
[optional SHA-256 of key, 32 B, if flags & 2]
[SimpleFileEOF for stream 0]
```

### HttpResponseInfo pickle (stream 0)

`u32 payload_size`, then `i32 flags`, `i64 request_time`, `i64 response_time`, …, then the raw
headers as NUL-separated lines (`HTTP/1.1 200\0Content-Type: image/png\0…\0\0`).

### Cache keys

Keys may be prefixed with the network isolation key, e.g.
`1/0/_dk_https://discord.com https://discord.com https://cdn.discordapp.com/attachments/…`.
The URL is the last whitespace-separated token.

## Recovered metadata

| URL | kind | metadata |
|---|---|---|
| `cdn.discordapp.com` / `media.discordapp.net` `/attachments/{channel}/{attachment}/{name}` | attachment | original filename, channel ID, posted date from the attachment snowflake |
| `/emojis/{id}` | emoji | |
| `/stickers/{id}` | sticker | |
| `/avatars/…`, `/guilds/{g}/users/…`, `/avatar-decoration-presets/…` | avatar | |
| `/icons/{guild}/…`, `/banners/`, `/splashes/`, `/role-icons/` | server | guild ID |
| `media.tenor.com`, `*.giphy.com`, `*.klipy.com` | gif | |
| `images-ext-*.discordapp.net/external/…/https/host/path` | embed | original source URL |
| `discord.com/assets/…`, `cdn.discordapp.com/assets/…` | interface | Discord's own UI images |
| other URL | embed | |
| no URL (orphan file) | unknown | |

Snowflake → Unix ms: `(id >> 22) + 1420070400000`.

## IPC contract

All JSON is camelCase.

```ts
type Kind = 'attachment' | 'gif' | 'emoji' | 'sticker' | 'avatar' | 'server' | 'embed' | 'interface' | 'unknown';

interface Entry {
  id: number;                 // unique per app run
  url: string | null;         // cache key URL
  originalUrl: string | null; // source URL for proxied embeds
  host: string | null;
  kind: Kind;
  fileName: string;           // best display/export name, always has an extension
  mime: string;               // e.g. image/png
  ext: string;                // e.g. png
  size: number;               // body bytes
  postedAt: number | null;    // Unix ms, from attachment snowflake
  cachedAt: number | null;    // Unix ms, when Discord downloaded it
  channelId: string | null;
  guildId: string | null;
  groupKey: string;           // same media at different sizes shares a key
  source: string;             // install label, e.g. "Discord Canary"
}

interface Source { label: string; path: string; format: 'blockfile' | 'simple' | 'unknown' }

type ScanEvent =
  | { type: 'started';  data: { total: number; sources: Source[] } }
  | { type: 'batch';    data: { entries: Entry[]; done: number; total: number } }
  | { type: 'finished'; data: {
      count: number; elapsedMs: number; errors: number;
      lockedSources: string[]; // sources whose index Discord held locked (Discord running)
      diagnostics: string;     // counts-only scan details for bug reports (no URLs/names)
    } };
```

Commands (`invoke`):

- `list_sources() -> Source[]`: detected Discord installs.
- `scan({ dirs: string[] | null, onEvent: Channel<ScanEvent> }) -> void`: `dirs: null` scans
  detected installs; otherwise scans the given cache folders. A new scan cancels the previous one.
- `export_entries({ ids: number[], dest: string }) -> { written: number; failed: number; dest: string }`.
  Writes bodies with unique, sanitized names and sets each file's modified time to
  `postedAt ?? cachedAt`.

Media: the `media` URI scheme. `convertFileSrc(String(id), 'media')` returns the URL
(`http://media.localhost/<id>` on Windows, `media://localhost/<id>` elsewhere). The response is
the body with its `Content-Type` and supports `Range` requests.

## Performance

- The scanner never reads whole media bodies. Per entry it reads the header + key + first
  64 body bytes, and the trailer (stream 0 + EOF) in one tail read.
- Block files are metadata and are read once per scan.
- Per-entry work runs on Tokio's blocking pool with bounded concurrency
  (`buffer_unordered`), and results stream to the UI in batches over a `Channel`.
