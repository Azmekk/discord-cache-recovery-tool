//! Chromium "blockfile" cache (Windows): `index`, `data_N` block files, `f_XXXXXX` files.
//!
//! We do not trust the `index` hash table (it may be stale or mid-update). Instead every
//! allocated 256-byte block of every entry block file is checked for a plausible
//! `EntryStore`, which also recovers entries the index no longer links.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::http::{base_time_to_unix_ms, parse_response_info, ResponseInfo};
use crate::io::{i32le, read_vec, u32le, u64le};

pub const BLOCK_MAGIC: u32 = 0xC104_CAC3;
pub const BLOCK_HEADER_SIZE: u64 = 8192;
const BITMAP_OFFSET: usize = 80;
const BITMAP_WORDS: usize = 2028;
const MAX_BLOCKS: usize = BITMAP_WORDS * 32;
pub const ENTRY_SIZE: usize = 256;
const KEY_OFFSET: usize = 96;
const MAX_KEY_LEN: usize = 8192;
/// `4 * sizeof(EntryStore) - offsetof(EntryStore, key) - 1`.
const MAX_INTERNAL_KEY_LEN: usize = 4 * ENTRY_SIZE - KEY_OFFSET - 1;
const MAX_STREAM0: u64 = 256 * 1024;

/// A decoded `CacheAddr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Addr {
    /// `f_%06x`
    External(u32),
    /// Blocks inside `data_<file>`.
    Block {
        file: u8,
        start: u32,
        count: u32,
        block_size: u32,
    },
}

impl Addr {
    pub fn decode(v: u32) -> Option<Addr> {
        if v & 0x8000_0000 == 0 {
            return None;
        }
        let ty = (v >> 28) & 0x7;
        if ty == 0 {
            return Some(Addr::External(v & 0x0FFF_FFFF));
        }
        let block_size = match ty {
            1 => 36,
            2 => 256,
            3 => 1024,
            4 => 4096,
            _ => return None,
        };
        if v & 0x0C00_0000 != 0 {
            return None; // reserved bits must be zero
        }
        Some(Addr::Block {
            file: ((v >> 16) & 0xFF) as u8,
            start: v & 0xFFFF,
            count: ((v >> 24) & 0x3) + 1,
            block_size,
        })
    }
}

pub fn external_name(n: u32) -> String {
    format!("f_{n:06x}")
}

/// A recovered entry with its body location (body bytes are not read here).
#[derive(Debug, Clone)]
pub struct BlockEntry {
    pub key: String,
    /// `EntryStore::creation_time`, Unix ms.
    pub creation_time: Option<i64>,
    pub info: ResponseInfo,
    pub body_path: PathBuf,
    pub body_offset: u64,
    pub body_len: u64,
    /// Shared read-only handle for the body file when it is a `data_N` file.
    pub body_handle: Option<Arc<File>>,
}

#[derive(Debug, Default)]
pub struct BlockfileScan {
    pub entries: Vec<BlockEntry>,
    /// File names (e.g. `f_00001a`) referenced by accepted entries.
    pub referenced: HashSet<String>,
    pub errors: u32,
    pub stats: Stats,
}

/// Counters for the scan-details report. Counts only: never keys, URLs or contents.
#[derive(Debug, Default, Clone)]
pub struct Stats {
    /// `data_N` files present in the folder.
    pub data_files: u32,
    /// `data_N` files another process (Discord) holds open exclusively.
    pub locked: u32,
    /// `data_N` files that failed to open or read for another reason.
    pub unreadable: u32,
    /// `data_N` files whose header marks them as 256-byte entry files.
    pub entry_files: u32,
    pub blocks_allocated: u64,
    pub accepted: u32,
    pub body_missing: u32,
    pub rej_state: u32,
    pub rej_key_len: u32,
    pub rej_time: u32,
    pub rej_body_addr: u32,
    pub rej_body_size: u32,
    pub rej_key: u32,
}

/// Windows ERROR_SHARING_VIOLATION / ERROR_LOCK_VIOLATION.
pub fn is_lock_error(e: &io::Error) -> bool {
    matches!(e.raw_os_error(), Some(32) | Some(33))
}

struct DataFile {
    file: Arc<File>,
    len: u64,
}

/// Cache of open `data_N` / `f_` handles during a walk.
struct Files<'a> {
    dir: &'a Path,
    data: HashMap<u8, Option<DataFile>>,
}

impl<'a> Files<'a> {
    fn data(&mut self, sel: u8) -> Option<&DataFile> {
        let dir = self.dir;
        self.data
            .entry(sel)
            .or_insert_with(|| {
                let file = crate::io::open_ro(&dir.join(format!("data_{sel}"))).ok()?;
                let len = file.metadata().ok()?.len();
                Some(DataFile {
                    file: Arc::new(file),
                    len,
                })
            })
            .as_ref()
    }

    /// Resolves an address + size to `(path, offset, len, handle)`. Returns None if the
    /// location is inconsistent (out of range, missing file, size larger than blocks).
    fn locate(&mut self, addr: Addr, size: u64) -> Option<(PathBuf, u64, u64, Option<Arc<File>>)> {
        match addr {
            Addr::External(n) => {
                let path = self.dir.join(external_name(n));
                let len = std::fs::metadata(&path).ok()?.len();
                Some((path, 0, size.min(len), None))
            }
            Addr::Block {
                file,
                start,
                count,
                block_size,
            } => {
                if size > (count * block_size) as u64 {
                    return None;
                }
                let offset = BLOCK_HEADER_SIZE + start as u64 * block_size as u64;
                let df = self.data(file)?;
                if offset + size > df.len {
                    return None;
                }
                let handle = df.file.clone();
                Some((
                    self.dir.join(format!("data_{file}")),
                    offset,
                    size,
                    Some(handle),
                ))
            }
        }
    }

    fn read(&mut self, addr: Addr, size: u64) -> Option<Vec<u8>> {
        let (path, offset, len, handle) = self.locate(addr, size)?;
        let buf = match handle {
            Some(h) => read_vec(&h, offset, len as usize).ok()?,
            None => read_vec(&crate::io::open_ro(&path).ok()?, offset, len as usize).ok()?,
        };
        Some(buf)
    }
}

/// `EntryImpl::NumBlocksForEntry`.
fn blocks_for_key(key_len: usize) -> usize {
    let key1 = ENTRY_SIZE - KEY_OFFSET;
    if key_len < key1 || key_len > MAX_INTERNAL_KEY_LEN {
        1
    } else {
        (key_len - key1) / ENTRY_SIZE + 2
    }
}

fn valid_key(k: &[u8]) -> bool {
    k.iter().all(|&c| (0x20..0x7F).contains(&c)) && k.windows(4).any(|w| w == b"http")
}

/// Lists the `data_N` files in `dir` whose header says they hold 256-byte blocks.
/// Files that can't be opened are counted in `stats` rather than silently skipped.
fn entry_block_files(dir: &Path, stats: &mut Stats) -> Vec<(u8, PathBuf)> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(sel) = name
            .strip_prefix("data_")
            .and_then(|n| n.parse::<u8>().ok())
        else {
            continue;
        };
        stats.data_files += 1;
        let path = e.path();
        let header = crate::io::open_ro(&path).and_then(|f| read_vec(&f, 0, 16));
        let h = match header {
            Ok(h) => h,
            Err(e) if is_lock_error(&e) => {
                stats.locked += 1;
                continue;
            }
            Err(_) => {
                stats.unreadable += 1;
                continue;
            }
        };
        if h.len() == 16 && u32le(&h, 0) == BLOCK_MAGIC && i32le(&h, 12) == ENTRY_SIZE as i32 {
            out.push((sel, path));
        }
    }
    stats.entry_files = out.len() as u32;
    out.sort();
    out
}

/// Walks all entry block files in `dir`. `cancelled` is polled between files/blocks.
pub fn parse_dir(dir: &Path, cancelled: &dyn Fn() -> bool) -> io::Result<BlockfileScan> {
    let mut scan = BlockfileScan::default();
    let mut files = Files {
        dir,
        data: HashMap::new(),
    };
    for (sel, path) in entry_block_files(dir, &mut scan.stats) {
        if cancelled() {
            break;
        }
        // Entry block files are metadata: read each once, whole.
        let bytes = match crate::io::open_ro(&path).and_then(|f| {
            let len = f.metadata()?.len() as usize;
            read_vec(&f, 0, len)
        }) {
            Ok(b) => b,
            Err(e) => {
                if is_lock_error(&e) {
                    scan.stats.locked += 1;
                } else {
                    scan.stats.unreadable += 1;
                }
                scan.errors += 1;
                continue;
            }
        };
        walk_file(sel, &bytes, &mut files, &mut scan, cancelled);
    }
    Ok(scan)
}

fn walk_file(
    sel: u8,
    bytes: &[u8],
    files: &mut Files,
    scan: &mut BlockfileScan,
    cancelled: &dyn Fn() -> bool,
) {
    if bytes.len() < BLOCK_HEADER_SIZE as usize {
        return;
    }
    let max_entries = i32le(bytes, 20).max(0) as usize;
    let in_file = (bytes.len() - BLOCK_HEADER_SIZE as usize) / ENTRY_SIZE;
    let limit = if max_entries > 0 {
        max_entries.min(in_file)
    } else {
        in_file
    }
    .min(MAX_BLOCKS);
    let allocated = |i: usize| (u32le(bytes, BITMAP_OFFSET + (i / 32) * 4) >> (i % 32)) & 1 == 1;

    let mut i = 0;
    while i < limit {
        if i % 4096 == 0 && cancelled() {
            return;
        }
        if !allocated(i) {
            i += 1;
            continue;
        }
        scan.stats.blocks_allocated += 1;
        let off = BLOCK_HEADER_SIZE as usize + i * ENTRY_SIZE;
        match parse_entry(bytes, off, limit - i, sel, files, scan) {
            Some(blocks) => i += blocks.max(1),
            None => i += 1,
        }
    }
}

/// Key: inline (possibly spanning contiguous blocks) or via `long_key`. Returns the key and
/// the number of blocks the entry spans.
fn read_key(
    bytes: &[u8],
    off: usize,
    blocks_left: usize,
    sel: u8,
    key_len: usize,
    long_key: u32,
    files: &mut Files,
) -> Option<(String, usize)> {
    if long_key == 0 {
        let blocks = blocks_for_key(key_len);
        if key_len > MAX_INTERNAL_KEY_LEN || blocks > blocks_left {
            return None;
        }
        let k = bytes.get(off + KEY_OFFSET..off + KEY_OFFSET + key_len)?;
        if !valid_key(k) {
            return None;
        }
        return Some((String::from_utf8_lossy(k).into_owned(), blocks));
    }
    let addr = Addr::decode(long_key)?;
    let k = match addr {
        Addr::Block { file, start, .. } if file == sel => {
            let o = BLOCK_HEADER_SIZE as usize + start as usize * ENTRY_SIZE;
            bytes.get(o..o + key_len)?.to_vec()
        }
        _ => files.read(addr, key_len as u64)?,
    };
    let k = k.strip_suffix(b"\0").unwrap_or(&k);
    if k.len() != key_len || !valid_key(k) {
        return None;
    }
    Some((String::from_utf8_lossy(k).into_owned(), 1))
}

/// Validates and resolves the `EntryStore` at `off`. Returns the number of blocks it
/// spans when accepted.
fn parse_entry(
    bytes: &[u8],
    off: usize,
    blocks_left: usize,
    sel: u8,
    files: &mut Files,
    scan: &mut BlockfileScan,
) -> Option<usize> {
    let e = &bytes[off..off + ENTRY_SIZE];
    let state = i32le(e, 20);
    let creation = u64le(e, 24);
    let key_len = i32le(e, 32);
    let long_key = u32le(e, 36);
    let data_size = |n: usize| i32le(e, 40 + n * 4);
    let data_addr = |n: usize| u32le(e, 56 + n * 4);

    let stats = &mut scan.stats;
    if state != 0 {
        stats.rej_state += 1;
        return None;
    }
    if key_len <= 0 || key_len as usize > MAX_KEY_LEN {
        stats.rej_key_len += 1;
        return None;
    }
    let key_len = key_len as usize;
    let Some(creation_time) = i64::try_from(creation).ok().and_then(base_time_to_unix_ms) else {
        stats.rej_time += 1;
        return None;
    };
    let body_addr = match Addr::decode(data_addr(1)) {
        // Rankings blocks (36 B) never hold data.
        Some(Addr::Block { block_size: 36, .. }) | None => {
            stats.rej_body_addr += 1;
            return None;
        }
        Some(a) => a,
    };
    let body_size = data_size(1);
    if body_size <= 0 {
        stats.rej_body_size += 1;
        return None;
    }

    let Some((key, blocks)) = read_key(bytes, off, blocks_left, sel, key_len, long_key, files)
    else {
        scan.stats.rej_key += 1;
        return None;
    };
    if let Some(Addr::External(n)) = Addr::decode(long_key) {
        scan.referenced.insert(external_name(n));
    }

    // Reference every external stream so orphan detection skips them.
    for n in 0..4 {
        if let Some(Addr::External(f)) = Addr::decode(data_addr(n)) {
            scan.referenced.insert(external_name(f));
        }
    }

    let Some((body_path, body_offset, body_len, body_handle)) =
        files.locate(body_addr, body_size as u64)
    else {
        scan.stats.body_missing += 1;
        scan.errors += 1;
        return Some(blocks);
    };
    scan.stats.accepted += 1;

    let info = match (Addr::decode(data_addr(0)), data_size(0)) {
        (Some(a), s) if s > 0 => files
            .read(a, (s as u64).min(MAX_STREAM0))
            .map(|b| parse_response_info(&b))
            .unwrap_or_default(),
        _ => ResponseInfo::default(),
    };

    scan.entries.push(BlockEntry {
        key,
        creation_time: Some(creation_time),
        info,
        body_path,
        body_offset,
        body_len,
        body_handle,
    });
    Some(blocks)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::http::tests::{build_pickle, unix_ms_to_base};

    pub fn addr_external(n: u32) -> u32 {
        0x8000_0000 | n
    }

    pub fn addr_block(ty: u32, file: u32, start: u32, count: u32) -> u32 {
        0x8000_0000 | (ty << 28) | ((count - 1) << 24) | (file << 16) | start
    }

    /// A block file with `n_blocks` blocks of `entry_size`, none allocated.
    pub fn new_block_file(this_file: i16, entry_size: i32, n_blocks: usize) -> Vec<u8> {
        let mut f = vec![0u8; BLOCK_HEADER_SIZE as usize + n_blocks * entry_size as usize];
        f[0..4].copy_from_slice(&BLOCK_MAGIC.to_le_bytes());
        f[4..8].copy_from_slice(&0x0003_0000u32.to_le_bytes());
        f[8..10].copy_from_slice(&this_file.to_le_bytes());
        f[12..16].copy_from_slice(&entry_size.to_le_bytes());
        f[20..24].copy_from_slice(&(n_blocks as i32).to_le_bytes());
        f
    }

    pub fn put_block(f: &mut [u8], entry_size: usize, start: usize, data: &[u8]) {
        let blocks = data.len().div_ceil(entry_size).max(1);
        for b in start..start + blocks {
            let w = BITMAP_OFFSET + (b / 32) * 4;
            let v = u32le(f, w) | (1 << (b % 32));
            f[w..w + 4].copy_from_slice(&v.to_le_bytes());
        }
        let o = BLOCK_HEADER_SIZE as usize + start * entry_size;
        f[o..o + data.len()].copy_from_slice(data);
    }

    pub struct EntrySpec<'a> {
        pub key: &'a str,
        pub created_ms: i64,
        pub s0: (u32, i32),
        pub body: (u32, i32),
        pub long_key: u32,
        pub state: i32,
    }

    pub fn entry_store(s: &EntrySpec) -> Vec<u8> {
        let inline = if s.long_key == 0 { s.key.len() + 1 } else { 0 };
        let mut e = vec![0u8; (KEY_OFFSET + inline).div_ceil(ENTRY_SIZE) * ENTRY_SIZE];
        e[0..4].copy_from_slice(&0x1234_5678u32.to_le_bytes());
        e[8..12].copy_from_slice(&addr_block(1, 0, 5, 1).to_le_bytes());
        e[20..24].copy_from_slice(&s.state.to_le_bytes());
        e[24..32].copy_from_slice(&unix_ms_to_base(s.created_ms).to_le_bytes());
        e[32..36].copy_from_slice(&(s.key.len() as i32).to_le_bytes());
        e[36..40].copy_from_slice(&s.long_key.to_le_bytes());
        e[40..44].copy_from_slice(&s.s0.1.to_le_bytes());
        e[44..48].copy_from_slice(&s.body.1.to_le_bytes());
        e[56..60].copy_from_slice(&s.s0.0.to_le_bytes());
        e[60..64].copy_from_slice(&s.body.0.to_le_bytes());
        if s.long_key == 0 {
            e[KEY_OFFSET..KEY_OFFSET + s.key.len()].copy_from_slice(s.key.as_bytes());
        }
        e
    }

    pub const GIF: &[u8] = b"GIF89a\x01\0\x01\0\x80\0\0\0\0\0\xff\xff\xff!\xf9\x04\x01\0\0\0\0,\0\0\0\0\x01\0\x01\0\0\x02\x02D\x01\0;";
    pub const PNG: &[u8] = crate::cache::simple::tests::PNG;
    pub const KEY_GIF: &str =
        "1/0/_dk_https://discord.com https://discord.com https://media.tenor.com/abc/tenor.gif";
    pub const KEY_PNG: &str = "https://cdn.discordapp.com/emojis/123456789012345678.png?size=48";
    /// A key long enough to spill into a second inline block.
    pub fn long_inline_key() -> String {
        format!(
            "1/0/_dk_https://discord.com https://discord.com https://cdn.discordapp.com/attachments/111111111111111111/1100000000000000000/{}.png",
            "x".repeat(120)
        )
    }
    pub const CREATED: i64 = 1_690_000_000_000;

    /// Builds a synthetic blockfile cache directory. Returns the response time used for
    /// the GIF entry's stream 0.
    pub fn build_blockfile_dir(dir: &Path) -> i64 {
        let resp_ms = 1_690_000_123_456;
        let mut d1 = new_block_file(1, 256, 64);
        let mut d2 = new_block_file(2, 1024, 16);
        std::fs::write(dir.join("index"), [0u8; 256]).unwrap();
        std::fs::write(dir.join("data_0"), new_block_file(0, 36, 16)).unwrap();
        std::fs::write(dir.join("data_3"), new_block_file(3, 4096, 4)).unwrap();

        // Entry 1: GIF body in external f_000001, stream 0 in data_2 block 0.
        let s0 = build_pickle(resp_ms, &["HTTP/1.1 200", "content-type: image/gif"]);
        put_block(&mut d2, 1024, 0, &s0);
        std::fs::write(dir.join("f_000001"), GIF).unwrap();
        let e1 = entry_store(&EntrySpec {
            key: KEY_GIF,
            created_ms: CREATED,
            s0: (addr_block(3, 2, 0, 1), s0.len() as i32),
            body: (addr_external(1), GIF.len() as i32),
            long_key: 0,
            state: 0,
        });
        put_block(&mut d1, 256, 2, &e1);

        // Entry 2: small PNG body inline in data_2 block 3 (no stream 0).
        put_block(&mut d2, 1024, 3, PNG);
        let e2 = entry_store(&EntrySpec {
            key: KEY_PNG,
            created_ms: CREATED + 1000,
            s0: (0, 0),
            body: (addr_block(3, 2, 3, 1), PNG.len() as i32),
            long_key: 0,
            state: 0,
        });
        put_block(&mut d1, 256, 4, &e2);

        // Entry 3: key spanning two blocks; body inline in a data_1 256-byte block.
        let k3 = long_inline_key();
        put_block(&mut d1, 256, 40, GIF);
        let e3 = entry_store(&EntrySpec {
            key: &k3,
            created_ms: CREATED + 2000,
            s0: (0, 0),
            body: (addr_block(2, 1, 40, 1), GIF.len() as i32),
            long_key: 0,
            state: 0,
        });
        assert_eq!(e3.len(), 512);
        put_block(&mut d1, 256, 10, &e3);

        // Junk allocated blocks that must be rejected.
        let mut junk = vec![0xAAu8; 256];
        junk[20..24].copy_from_slice(&0i32.to_le_bytes());
        put_block(&mut d1, 256, 20, &junk); // garbage
        let doomed = entry_store(&EntrySpec {
            key: "https://cdn.discordapp.com/emojis/9.png",
            created_ms: CREATED,
            s0: (0, 0),
            body: (addr_external(1), 10),
            long_key: 0,
            state: 2,
        });
        put_block(&mut d1, 256, 21, &doomed); // doomed state
        let ancient = entry_store(&EntrySpec {
            key: "https://cdn.discordapp.com/emojis/9.png",
            created_ms: 100_000,
            s0: (0, 0),
            body: (addr_external(1), 10),
            long_key: 0,
            state: 0,
        });
        put_block(&mut d1, 256, 22, &ancient); // implausible creation time
        let binkey = entry_store(&EntrySpec {
            key: "\x01\x02 not a url",
            created_ms: CREATED,
            s0: (0, 0),
            body: (addr_external(1), 10),
            long_key: 0,
            state: 0,
        });
        put_block(&mut d1, 256, 23, &binkey); // key not printable / no http
        let nobody = entry_store(&EntrySpec {
            key: "https://cdn.discordapp.com/emojis/9.png",
            created_ms: CREATED,
            s0: (0, 0),
            body: (0, 0),
            long_key: 0,
            state: 0,
        });
        put_block(&mut d1, 256, 24, &nobody); // body address not initialized
                                              // A valid-looking entry in an UNALLOCATED block must be ignored.
        let unalloc = entry_store(&EntrySpec {
            key: "https://cdn.discordapp.com/emojis/7.png",
            created_ms: CREATED,
            s0: (0, 0),
            body: (addr_external(1), GIF.len() as i32),
            long_key: 0,
            state: 0,
        });
        let o = BLOCK_HEADER_SIZE as usize + 30 * 256;
        d1[o..o + 256].copy_from_slice(&unalloc);

        std::fs::write(dir.join("data_1"), d1).unwrap();
        std::fs::write(dir.join("data_2"), d2).unwrap();
        // Unreferenced external file (orphan).
        std::fs::write(dir.join("f_000002"), PNG).unwrap();
        resp_ms
    }

    #[test]
    fn decodes_addresses() {
        assert_eq!(Addr::decode(0), None);
        assert_eq!(Addr::decode(0x8000_002A), Some(Addr::External(42)));
        assert_eq!(
            Addr::decode(addr_block(3, 2, 7, 4)),
            Some(Addr::Block {
                file: 2,
                start: 7,
                count: 4,
                block_size: 1024
            })
        );
        assert_eq!(Addr::decode(0xD000_0000), None); // type 5
        assert_eq!(blocks_for_key(10), 1);
        assert_eq!(blocks_for_key(159), 1);
        assert_eq!(blocks_for_key(160), 2);
        assert_eq!(blocks_for_key(416), 3);
        assert_eq!(blocks_for_key(927), 4);
    }

    #[test]
    fn walks_synthetic_blockfile_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let resp_ms = build_blockfile_dir(tmp.path());
        let scan = parse_dir(tmp.path(), &|| false).unwrap();
        assert_eq!(scan.errors, 0);
        let keys: Vec<&str> = scan.entries.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec![KEY_GIF, KEY_PNG, long_inline_key().as_str()]);

        let gif = &scan.entries[0];
        assert_eq!(gif.body_path, tmp.path().join("f_000001"));
        assert_eq!((gif.body_offset, gif.body_len), (0, GIF.len() as u64));
        assert_eq!(gif.creation_time, Some(CREATED));
        assert_eq!(gif.info.response_time, Some(resp_ms));
        assert_eq!(gif.info.content_type.as_deref(), Some("image/gif"));

        let png = &scan.entries[1];
        assert_eq!(png.body_path, tmp.path().join("data_2"));
        assert_eq!(png.body_offset, 8192 + 3 * 1024);
        assert_eq!(png.body_len, PNG.len() as u64);
        assert!(png.body_handle.is_some());
        assert_eq!(png.info, ResponseInfo::default());

        let small = &scan.entries[2];
        assert_eq!(small.body_path, tmp.path().join("data_1"));
        assert_eq!(small.body_offset, 8192 + 40 * 256);

        assert!(scan.referenced.contains("f_000001"));
        assert!(!scan.referenced.contains("f_000002"));
    }

    #[test]
    fn resolves_long_key() {
        let tmp = tempfile::tempdir().unwrap();
        let key = format!(
            "https://cdn.discordapp.com/attachments/1/2/{}.png",
            "y".repeat(1000)
        );
        let mut d1 = new_block_file(1, 256, 32);
        let mut d3 = new_block_file(3, 4096, 4);
        let mut kb = key.as_bytes().to_vec();
        kb.push(0);
        put_block(&mut d3, 4096, 1, &kb);
        put_block(&mut d3, 4096, 2, PNG);
        let e = entry_store(&EntrySpec {
            key: &key,
            created_ms: CREATED,
            s0: (0, 0),
            body: (addr_block(4, 3, 2, 1), PNG.len() as i32),
            long_key: addr_block(4, 3, 1, 1),
            state: 0,
        });
        put_block(&mut d1, 256, 0, &e);
        std::fs::write(tmp.path().join("data_1"), d1).unwrap();
        std::fs::write(tmp.path().join("data_3"), d3).unwrap();
        let scan = parse_dir(tmp.path(), &|| false).unwrap();
        assert_eq!(scan.entries.len(), 1);
        assert_eq!(scan.entries[0].key, key);
        assert_eq!(scan.entries[0].body_offset, 8192 + 2 * 4096);
    }
}
