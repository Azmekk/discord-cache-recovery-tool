//! The `media` URI scheme: serves entry bodies by id, with Range support.

use std::sync::Arc;

use tauri::http::{header, Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

use crate::io::{open_ro, read_vec};
use crate::model::{AppState, Location};

/// Max bytes returned for an open-ended range (`bytes=a-`).
pub const MAX_CHUNK: u64 = 8 * 1024 * 1024;

/// Parses a `Range` header against a body of `len` bytes into an inclusive `(start, end)`.
/// Only the first range of a multi-range request is honored.
pub fn parse_range(value: &str, len: u64) -> Option<(u64, u64)> {
    let spec = value.trim().strip_prefix("bytes=")?;
    let first = spec.split(',').next()?.trim();
    let (a, b) = first.split_once('-')?;
    let (a, b) = (a.trim(), b.trim());
    if len == 0 {
        return None;
    }
    if a.is_empty() {
        let n: u64 = b.parse().ok()?;
        if n == 0 {
            return None;
        }
        return Some((len - n.min(len), len - 1));
    }
    let start: u64 = a.parse().ok()?;
    if start >= len {
        return None;
    }
    let end = if b.is_empty() {
        (start + MAX_CHUNK - 1).min(len - 1)
    } else {
        let e: u64 = b.parse().ok()?;
        if e < start {
            return None;
        }
        e.min(len - 1)
    };
    Some((start, end))
}

fn base(status: StatusCode) -> tauri::http::response::Builder {
    Response::builder()
        .status(status)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::CACHE_CONTROL, "no-store")
}

fn empty(status: StatusCode) -> Response<Vec<u8>> {
    base(status).body(Vec::new()).unwrap()
}

/// Pure request handler: `path` is the URI path (`/<id>`), `range` the Range header.
pub fn handle(
    state: &AppState,
    method: &str,
    path: &str,
    range: Option<&str>,
) -> Response<Vec<u8>> {
    if method.eq_ignore_ascii_case("OPTIONS") {
        return base(StatusCode::NO_CONTENT)
            .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "Range")
            .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, HEAD, OPTIONS")
            .body(Vec::new())
            .unwrap();
    }
    let id = path
        .trim_start_matches('/')
        .split(['/', '?'])
        .next()
        .unwrap_or("");
    let Some((loc, mime)) = id.parse::<u32>().ok().and_then(|id| lookup(state, id)) else {
        return empty(StatusCode::NOT_FOUND);
    };
    let len = loc.len;
    let (status, start, end) = match range {
        Some(r) => match parse_range(r, len) {
            Some((s, e)) => (StatusCode::PARTIAL_CONTENT, s, e),
            None => {
                return base(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(header::CONTENT_RANGE, format!("bytes */{len}"))
                    .body(Vec::new())
                    .unwrap()
            }
        },
        None if len == 0 => (StatusCode::OK, 0, 0),
        None => (StatusCode::OK, 0, len - 1),
    };
    let want = if len == 0 { 0 } else { end - start + 1 };
    let body = if method.eq_ignore_ascii_case("HEAD") {
        Vec::new()
    } else {
        match open_ro(&loc.path).and_then(|f| read_vec(&f, loc.offset + start, want as usize)) {
            Ok(b) if b.len() as u64 == want => b,
            _ => return empty(StatusCode::INTERNAL_SERVER_ERROR),
        }
    };
    let mut b = base(status)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CONTENT_LENGTH, want.to_string())
        .header(header::ACCEPT_RANGES, "bytes");
    if status == StatusCode::PARTIAL_CONTENT {
        b = b.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}"));
    }
    b.body(body).unwrap()
}

fn lookup(state: &AppState, id: u32) -> Option<(Location, String)> {
    let map = state.entries.read().ok()?;
    let e = map.get(&id)?;
    Some((e.loc.clone(), e.entry.mime.clone()))
}

/// Handler for `register_asynchronous_uri_scheme_protocol("media", ..)`.
pub fn protocol<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let state: Arc<AppState> = ctx.app_handle().state::<Arc<AppState>>().inner().clone();
    let method = request.method().as_str().to_string();
    let path = request.uri().path().to_string();
    let range = request
        .headers()
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    tauri::async_runtime::spawn_blocking(move || {
        responder.respond(handle(&state, &method, &path, range.as_deref()));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_parsing() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(parse_range("bytes=100-", 1000), Some((100, 999)));
        assert_eq!(parse_range("bytes=-100", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=-5000", 1000), Some((0, 999)));
        assert_eq!(parse_range("bytes=500-5000", 1000), Some((500, 999)));
        assert_eq!(parse_range("bytes=0-1, 5-6", 1000), Some((0, 1)));
        assert_eq!(
            parse_range("bytes=0-", 100 * MAX_CHUNK),
            Some((0, MAX_CHUNK - 1))
        );
        assert_eq!(parse_range("bytes=1000-", 1000), None);
        assert_eq!(parse_range("bytes=5-2", 1000), None);
        assert_eq!(parse_range("bytes=-0", 1000), None);
        assert_eq!(parse_range("items=0-1", 1000), None);
        assert_eq!(parse_range("bytes=x-1", 1000), None);
        assert_eq!(parse_range("bytes=0-0", 0), None);
    }

    fn scanned() -> (tempfile::TempDir, Arc<AppState>, crate::model::Entry) {
        let cache = tempfile::tempdir().unwrap();
        crate::cache::blockfile::tests::build_blockfile_dir(cache.path());
        let state = Arc::new(AppState::default());
        let entries = crate::scan::tests::entries_of(&crate::scan::tests::scan_dir(
            state.clone(),
            cache.path(),
        ));
        let gif = entries
            .into_iter()
            .find(|e| e.file_name == "tenor.gif")
            .unwrap();
        (cache, state, gif)
    }

    #[test]
    fn serves_full_and_partial_bodies() {
        let (_dir, state, gif) = scanned();
        let full_body = crate::cache::blockfile::tests::GIF;
        let path = format!("/{}", gif.id);

        let r = handle(&state, "GET", &path, None);
        assert_eq!(r.status(), 200);
        assert_eq!(r.headers()[header::CONTENT_TYPE], "image/gif");
        assert_eq!(
            r.headers()[header::CONTENT_LENGTH],
            full_body.len().to_string()
        );
        assert_eq!(r.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
        assert_eq!(r.body(), full_body);

        let r = handle(&state, "GET", &path, Some("bytes=2-5"));
        assert_eq!(r.status(), 206);
        assert_eq!(
            r.headers()[header::CONTENT_RANGE],
            format!("bytes 2-5/{}", full_body.len())
        );
        assert_eq!(r.body(), &full_body[2..6]);

        let r = handle(&state, "GET", &path, Some("bytes=9999-"));
        assert_eq!(r.status(), 416);
        assert_eq!(handle(&state, "GET", "/424242", None).status(), 404);
        assert_eq!(handle(&state, "GET", "/abc", None).status(), 404);
    }
}
