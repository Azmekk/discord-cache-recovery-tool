//! Chromium "simple" cache: one `<16 hex>_0` file per entry.
//!
//! ```text
//! [SimpleFileHeader 24 B][key][stream 1 = body][SimpleFileEOF 24 B]
//! [stream 0 = HttpResponseInfo][SHA-256(key) 32 B if flags & 2][SimpleFileEOF 24 B]
//! ```

use std::fs::File;
use std::io;
use std::path::Path;

use crate::http::{parse_response_info, ResponseInfo};
use crate::io::{read_vec, u32le, u64le};
use crate::sniff::SNIFF_LEN;

pub const HEADER_MAGIC: u64 = 0xfcfb_6d1b_a772_5c30;
pub const EOF_MAGIC: u64 = 0xf4fa_6f45_970d_41d8;
pub const HEADER_SIZE: u64 = 24;
pub const EOF_SIZE: u64 = 24;
pub const FLAG_HAS_KEY_SHA256: u32 = 2;
const MAX_KEY_LEN: u64 = 64 * 1024;
/// First read: header + key + body head for typical key lengths.
const HEAD_READ: usize = 1024;
/// Tail read: EOF0 + stream 0 + EOF1 for typical header sizes.
const TAIL_READ: u64 = 8 * 1024;

#[derive(Debug, Clone)]
pub struct SimpleEntry {
    pub key: String,
    pub body_offset: u64,
    pub body_len: u64,
    /// First ≤64 body bytes.
    pub head: Vec<u8>,
    pub info: ResponseInfo,
    /// Whether the trailer was valid (otherwise body = rest of file, info empty).
    pub trailer_ok: bool,
}

/// Returns true for names like `0123456789abcdef_0`.
pub fn is_entry_file_name(name: &str) -> bool {
    name.len() == 18 && name.ends_with("_0") && name[..16].bytes().all(|c| c.is_ascii_hexdigit())
}

pub fn parse_path(path: &Path) -> io::Result<SimpleEntry> {
    let file = crate::io::open_ro(path)?;
    let len = file.metadata()?.len();
    parse(&file, len)
}

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

pub fn parse(file: &File, len: u64) -> io::Result<SimpleEntry> {
    if len < HEADER_SIZE {
        return Err(invalid("too short for a simple cache header"));
    }
    // Read 1: header + key + first body bytes.
    let mut head = read_vec(file, 0, HEAD_READ.min(len as usize))?;
    if head.len() < HEADER_SIZE as usize || u64le(&head, 0) != HEADER_MAGIC {
        return Err(invalid("bad simple cache magic"));
    }
    let key_len = u32le(&head, 12) as u64;
    if key_len > MAX_KEY_LEN || HEADER_SIZE + key_len > len {
        return Err(invalid("bad key length"));
    }
    let body_offset = HEADER_SIZE + key_len;
    let want = (body_offset as usize + SNIFF_LEN).min(len as usize);
    if want > head.len() {
        head = read_vec(file, 0, want)?; // very long key: rare
    }
    let key = String::from_utf8_lossy(&head[HEADER_SIZE as usize..body_offset as usize])
        .trim_end_matches('\0')
        .to_string();

    // Read 2: tail with EOF0 + stream 0 + EOF1.
    let tail_len = len.min(TAIL_READ);
    let tail_start = len - tail_len;
    let tail = read_vec(file, tail_start, tail_len as usize)?;

    let (body_len, info, trailer_ok) =
        match parse_trailer(file, len, body_offset, tail_start, &tail)? {
            Some((body_len, info)) => (body_len, info, true),
            None => (len - body_offset, ResponseInfo::default(), false),
        };

    let head_end = (body_offset + body_len.min(SNIFF_LEN as u64)) as usize;
    let body_head = head[body_offset as usize..head_end.min(head.len())].to_vec();
    Ok(SimpleEntry {
        key,
        body_offset,
        body_len,
        head: body_head,
        info,
        trailer_ok,
    })
}

/// Returns `(body_len, response_info)` when the trailer is consistent.
fn parse_trailer(
    file: &File,
    len: u64,
    body_offset: u64,
    tail_start: u64,
    tail: &[u8],
) -> io::Result<Option<(u64, ResponseInfo)>> {
    let at = |abs: u64| -> Option<usize> { abs.checked_sub(tail_start).map(|v| v as usize) };
    if len < body_offset + 2 * EOF_SIZE {
        return Ok(None);
    }
    let eof0 = at(len - EOF_SIZE).unwrap();
    if u64le(tail, eof0) != EOF_MAGIC {
        return Ok(None);
    }
    let flags = u32le(tail, eof0 + 8);
    let s0_size = u32le(tail, eof0 + 16) as u64;
    let sha = if flags & FLAG_HAS_KEY_SHA256 != 0 {
        32
    } else {
        0
    };
    let Some(s0_off) = len.checked_sub(EOF_SIZE + sha + s0_size) else {
        return Ok(None);
    };
    let Some(eof1_off) = s0_off.checked_sub(EOF_SIZE) else {
        return Ok(None);
    };
    if eof1_off < body_offset {
        return Ok(None);
    }
    // EOF1 and stream 0 normally sit inside the tail read; read separately if not.
    let (eof1_ok, s0) = match at(eof1_off) {
        Some(i) => (
            u64le(tail, i) == EOF_MAGIC,
            tail[i + EOF_SIZE as usize..i + (EOF_SIZE + s0_size) as usize].to_vec(),
        ),
        None => {
            let buf = read_vec(file, eof1_off, (EOF_SIZE + s0_size) as usize)?;
            if buf.len() < (EOF_SIZE + s0_size) as usize {
                return Ok(None);
            }
            (
                u64le(&buf, 0) == EOF_MAGIC,
                buf[EOF_SIZE as usize..].to_vec(),
            )
        }
    };
    if !eof1_ok {
        return Ok(None);
    }
    Ok(Some((eof1_off - body_offset, parse_response_info(&s0))))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::http::tests::build_pickle;

    pub const PNG: &[u8] =
        b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0\x1f\x15\xc4\x89";

    /// Builds a simple-cache `_0` file byte by byte.
    pub fn build_simple(key: &str, body: &[u8], s0: &[u8], with_sha: bool) -> Vec<u8> {
        let mut f = Vec::new();
        f.extend_from_slice(&HEADER_MAGIC.to_le_bytes());
        f.extend_from_slice(&5u32.to_le_bytes()); // version
        f.extend_from_slice(&(key.len() as u32).to_le_bytes());
        f.extend_from_slice(&0xdeadbeefu32.to_le_bytes()); // key hash
        f.extend_from_slice(&0u32.to_le_bytes()); // padding
        f.extend_from_slice(key.as_bytes());
        f.extend_from_slice(body);
        // EOF for stream 1
        f.extend_from_slice(&EOF_MAGIC.to_le_bytes());
        f.extend_from_slice(&1u32.to_le_bytes()); // FLAG_HAS_CRC32
        f.extend_from_slice(&0x1234u32.to_le_bytes());
        f.extend_from_slice(&(body.len() as u32).to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(s0);
        if with_sha {
            f.extend_from_slice(&[0xAB; 32]);
        }
        f.extend_from_slice(&EOF_MAGIC.to_le_bytes());
        let flags = 1 | if with_sha { FLAG_HAS_KEY_SHA256 } else { 0 };
        f.extend_from_slice(&flags.to_le_bytes());
        f.extend_from_slice(&0x5678u32.to_le_bytes());
        f.extend_from_slice(&(s0.len() as u32).to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes());
        f
    }

    fn parse_bytes(bytes: &[u8]) -> SimpleEntry {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, bytes).unwrap();
        parse_path(tmp.path()).unwrap()
    }

    #[test]
    fn parses_simple_entry_with_and_without_sha() {
        let key = "1/0/_dk_https://discord.com https://discord.com https://cdn.discordapp.com/attachments/1/2/a.png";
        let ms = 1_710_000_000_000;
        let s0 = build_pickle(ms, &["HTTP/1.1 200", "Content-Type: image/png"]);
        for with_sha in [false, true] {
            let e = parse_bytes(&build_simple(key, PNG, &s0, with_sha));
            assert!(e.trailer_ok);
            assert_eq!(e.key, key);
            assert_eq!(e.body_offset, 24 + key.len() as u64);
            assert_eq!(e.body_len, PNG.len() as u64);
            assert_eq!(e.head, PNG);
            assert_eq!(crate::sniff::sniff(&e.head).unwrap().mime, "image/png");
            assert_eq!(e.info.response_time, Some(ms));
            assert_eq!(e.info.content_type.as_deref(), Some("image/png"));
        }
    }

    #[test]
    fn large_body_reads_only_head() {
        let key = "https://media.discordapp.net/attachments/1/2/big.png";
        let mut body = PNG.to_vec();
        body.resize(200_000, 7);
        let s0 = build_pickle(1_710_000_000_000, &["HTTP/1.1 200"]);
        let e = parse_bytes(&build_simple(key, &body, &s0, true));
        assert_eq!(e.body_len, 200_000);
        assert_eq!(e.head.len(), SNIFF_LEN);
    }

    #[test]
    fn invalid_trailer_falls_back_to_rest_of_file() {
        let key = "https://cdn.discordapp.com/emojis/1.png";
        let mut bytes = build_simple(key, PNG, &build_pickle(1_710_000_000_000, &[]), false);
        let n = bytes.len();
        bytes[n - 24] ^= 0xFF; // corrupt EOF0 magic
        let e = parse_bytes(&bytes);
        assert!(!e.trailer_ok);
        assert_eq!(e.body_len, (n - 24 - key.len()) as u64);
        assert_eq!(e.head.len(), SNIFF_LEN);
        assert!(e.head.starts_with(PNG));
    }

    #[test]
    fn rejects_non_cache_files() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, &[0u8; 100]).unwrap();
        assert!(parse_path(tmp.path()).is_err());
        assert!(is_entry_file_name("0123456789abcdef_0"));
        assert!(!is_entry_file_name("0123456789abcdef_1"));
        assert!(!is_entry_file_name("index"));
    }
}
