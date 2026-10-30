//! Parsing of stream 0: the pickled `net::HttpResponseInfo`.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::io::{i32le, i64le};

/// Microseconds between 1601-01-01 and 1970-01-01.
const WINDOWS_EPOCH_DELTA_US: i64 = 11_644_473_600_000_000;
/// 2015-01-01T00:00:00Z in Unix ms. Discord's cache cannot predate this.
const MIN_PLAUSIBLE_MS: i64 = 1_420_070_400_000;

/// What we extract from stream 0.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ResponseInfo {
    /// Unix ms when the response was received.
    pub response_time: Option<i64>,
    pub status: Option<u16>,
    pub content_type: Option<String>,
    /// `Date` header, Unix ms.
    pub date: Option<i64>,
    /// `Last-Modified` header, Unix ms.
    pub last_modified: Option<i64>,
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Accepts a Unix ms timestamp only if it lies in [2015, now + 2 days].
pub fn plausible_ms(ms: i64) -> Option<i64> {
    (MIN_PLAUSIBLE_MS..=now_ms() + 2 * 86_400_000)
        .contains(&ms)
        .then_some(ms)
}

/// Converts a `base::Time` internal value (µs since 1601) to plausible Unix ms.
pub fn base_time_to_unix_ms(t: i64) -> Option<i64> {
    if t <= WINDOWS_EPOCH_DELTA_US {
        return None;
    }
    plausible_ms((t - WINDOWS_EPOCH_DELTA_US) / 1000)
}

/// Parses a stream 0 blob (pickle header included).
pub fn parse_response_info(s0: &[u8]) -> ResponseInfo {
    let mut info = ResponseInfo::default();
    // Pickle: u32 payload_size, then i32 flags, i64 request_time, i64 response_time.
    // Newer Chromium versions may insert an extra i32 flags word after `flags`
    // (signalled by the top bit of `flags`), shifting the times by 4 bytes, so we
    // try the classic offset first and then the shifted one.
    if s0.len() >= 24 {
        let flags = i32le(s0, 4);
        let offsets: &[usize] = if flags < 0 { &[20, 16] } else { &[16, 20] };
        info.response_time = offsets
            .iter()
            .filter(|&&o| s0.len() >= o + 8)
            .find_map(|&o| base_time_to_unix_ms(i64le(s0, o)));
    }
    parse_headers(s0, &mut info);
    info
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn parse_headers(s0: &[u8], info: &mut ResponseInfo) {
    let Some(start) = find(s0, b"HTTP/") else {
        return;
    };
    let rest = &s0[start..];
    let end = find(rest, b"\0\0").unwrap_or(rest.len());
    let mut lines = rest[..end].split(|&c| c == 0);
    if let Some(status_line) = lines.next() {
        let s = String::from_utf8_lossy(status_line);
        info.status = s.split_whitespace().nth(1).and_then(|c| c.parse().ok());
    }
    for line in lines {
        let line = String::from_utf8_lossy(line);
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match name.trim().to_ascii_lowercase().as_str() {
            "content-type" if info.content_type.is_none() => {
                info.content_type = Some(value.to_string())
            }
            "date" => info.date = parse_http_date(value),
            "last-modified" => info.last_modified = parse_http_date(value),
            _ => {}
        }
    }
}

fn parse_http_date(v: &str) -> Option<i64> {
    let t = httpdate::parse_http_date(v).ok()?;
    let ms = t.duration_since(UNIX_EPOCH).ok()?.as_millis() as i64;
    plausible_ms(ms)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Unix ms → base::Time internal value.
    pub fn unix_ms_to_base(ms: i64) -> i64 {
        ms * 1000 + WINDOWS_EPOCH_DELTA_US
    }

    /// Builds a minimal HttpResponseInfo pickle.
    pub fn build_pickle(response_ms: i64, headers: &[&str]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&0x0001_0003i32.to_le_bytes()); // flags (version 3)
        payload.extend_from_slice(&unix_ms_to_base(response_ms - 120).to_le_bytes()); // request
        payload.extend_from_slice(&unix_ms_to_base(response_ms).to_le_bytes()); // response
        let mut raw = String::new();
        for h in headers {
            raw.push_str(h);
            raw.push('\0');
        }
        raw.push('\0');
        payload.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        payload.extend_from_slice(raw.as_bytes());
        while payload.len() % 4 != 0 {
            payload.push(0);
        }
        let mut out = (payload.len() as u32).to_le_bytes().to_vec();
        out.extend(payload);
        out
    }

    #[test]
    fn parses_times_and_headers() {
        let ms = 1_700_000_000_123;
        let p = build_pickle(
            ms,
            &[
                "HTTP/1.1 200",
                "content-type: image/png",
                "date: Tue, 14 Nov 2023 22:13:20 GMT",
                "last-modified: Mon, 13 Nov 2023 10:00:00 GMT",
            ],
        );
        let info = parse_response_info(&p);
        assert_eq!(info.response_time, Some(ms));
        assert_eq!(info.status, Some(200));
        assert_eq!(info.content_type.as_deref(), Some("image/png"));
        assert_eq!(info.date, Some(1_700_000_000_000));
        assert_eq!(info.last_modified, Some(1_699_869_600_000));
    }

    #[test]
    fn handles_extra_flags_layout() {
        let ms = 1_650_000_000_000;
        let mut s0 = Vec::new();
        s0.extend_from_slice(&40u32.to_le_bytes());
        s0.extend_from_slice(&(i32::MIN | 3).to_le_bytes());
        s0.extend_from_slice(&1i32.to_le_bytes());
        s0.extend_from_slice(&unix_ms_to_base(ms - 5).to_le_bytes());
        s0.extend_from_slice(&unix_ms_to_base(ms).to_le_bytes());
        s0.extend_from_slice(b"HTTP/1.1 200 OK\0\0");
        assert_eq!(parse_response_info(&s0).response_time, Some(ms));
    }

    #[test]
    fn rejects_implausible_times() {
        assert_eq!(base_time_to_unix_ms(0), None);
        assert_eq!(base_time_to_unix_ms(unix_ms_to_base(1_000)), None);
        assert_eq!(
            base_time_to_unix_ms(unix_ms_to_base(now_ms() + 30 * 86_400_000)),
            None
        );
        let info = parse_response_info(&[0u8; 30]);
        assert_eq!(info, ResponseInfo::default());
    }
}
