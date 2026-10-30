//! Exporting entry bodies to a folder.

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::StreamExt;
use tauri::State;

use crate::io::{open_ro, read_at};
use crate::model::{AppState, ExportResult, Location};

const MAX_NAME_CHARS: usize = 150;
const COPY_CONCURRENCY: usize = 8;
const CHUNK: usize = 1 << 20;

const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Splits `name` into a sanitized `(base, ext)` that is valid on Windows, macOS and Linux.
pub fn sanitize(name: &str, ext: &str) -> (String, String) {
    let clean = |s: &str| -> String {
        s.chars()
            .map(|c| match c {
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
                c if (c as u32) < 0x20 || c == '\u{7f}' => '_',
                c => c,
            })
            .collect()
    };
    let ext = clean(ext)
        .trim_matches(|c| c == '.' || c == ' ')
        .to_string();
    let ext = if ext.is_empty() {
        "bin".to_string()
    } else {
        ext
    };
    let base = match name.len().checked_sub(ext.len() + 1) {
        Some(i)
            if name.is_char_boundary(i) && name[i..].eq_ignore_ascii_case(&format!(".{ext}")) =>
        {
            &name[..i]
        }
        _ => name,
    };
    let mut base = clean(base).trim().trim_end_matches(['.', ' ']).to_string();
    let max_base = MAX_NAME_CHARS
        .saturating_sub(ext.chars().count() + 1)
        .max(1);
    if base.chars().count() > max_base {
        base = base.chars().take(max_base).collect::<String>();
        base = base.trim_end_matches(['.', ' ']).to_string();
    }
    if base.is_empty() {
        base = "file".to_string();
    }
    let stem = base.split('.').next().unwrap_or("").trim_end();
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        base = format!("_{base}");
    }
    (base, ext)
}

/// Assigns unique, sanitized names in order. Names are unique case-insensitively among
/// themselves and never clobber a file already present in `dest`.
pub fn assign_names(items: &[(String, String)], dest: &Path) -> Vec<String> {
    let mut taken: HashSet<String> = HashSet::new();
    items
        .iter()
        .map(|(name, ext)| {
            let (base, ext) = sanitize(name, ext);
            let mut n = 1;
            loop {
                let cand = if n == 1 {
                    format!("{base}.{ext}")
                } else {
                    format!("{base} ({n}).{ext}")
                };
                if !taken.contains(&cand.to_lowercase()) && !dest.join(&cand).exists() {
                    taken.insert(cand.to_lowercase());
                    break cand;
                }
                n += 1;
            }
        })
        .collect()
}

/// Copies exactly `[offset, offset + len)` of the source into a new file at `out`.
pub fn copy_body(loc: &Location, out: &Path, mtime_ms: Option<i64>) -> io::Result<()> {
    let src = open_ro(&loc.path)?;
    let mut dst = OpenOptions::new().write(true).create_new(true).open(out)?;
    let res = copy_range(&src, loc.offset, loc.len, &mut dst);
    drop(dst);
    if let Err(e) = res {
        let _ = std::fs::remove_file(out);
        return Err(e);
    }
    if let Some(ms) = mtime_ms {
        let ft = filetime::FileTime::from_unix_time(
            ms.div_euclid(1000),
            (ms.rem_euclid(1000) * 1_000_000) as u32,
        );
        let _ = filetime::set_file_mtime(out, ft);
    }
    Ok(())
}

fn copy_range(src: &File, offset: u64, len: u64, dst: &mut File) -> io::Result<()> {
    let mut buf = vec![0u8; CHUNK.min(len as usize).max(1)];
    let mut done = 0u64;
    while done < len {
        let want = buf.len().min((len - done) as usize);
        let n = read_at(src, offset + done, &mut buf[..want])?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "cache file truncated",
            ));
        }
        dst.write_all(&buf[..n])?;
        done += n as u64;
    }
    dst.flush()
}

struct Item {
    loc: Location,
    out: PathBuf,
    mtime: Option<i64>,
}

pub async fn export(
    state: Arc<AppState>,
    ids: Vec<u32>,
    dest: PathBuf,
) -> Result<ExportResult, String> {
    let d = dest.clone();
    tauri::async_runtime::spawn_blocking(move || std::fs::create_dir_all(&d))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("cannot create {}: {e}", dest.display()))?;

    let mut failed = 0u32;
    let found: Vec<(Location, Option<i64>, String, String)> = {
        let map = state.entries.read().map_err(|e| e.to_string())?;
        ids.iter()
            .filter_map(|id| {
                let e = map.get(id);
                if e.is_none() {
                    failed += 1;
                }
                e.map(|e| {
                    (
                        e.loc.clone(),
                        e.entry.posted_at.or(e.entry.cached_at),
                        e.entry.file_name.clone(),
                        e.entry.ext.clone(),
                    )
                })
            })
            .collect()
    };
    let d = dest.clone();
    let items: Vec<Item> = tauri::async_runtime::spawn_blocking(move || {
        let names: Vec<(String, String)> =
            found.iter().map(|f| (f.2.clone(), f.3.clone())).collect();
        let names = assign_names(&names, &d);
        found
            .into_iter()
            .zip(names)
            .map(|((loc, mtime, _, _), n)| Item {
                loc,
                out: d.join(n),
                mtime,
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())?;

    let results: Vec<bool> = futures::stream::iter(items)
        .map(|it| {
            tauri::async_runtime::spawn_blocking(move || {
                copy_body(&it.loc, &it.out, it.mtime).is_ok()
            })
        })
        .buffer_unordered(COPY_CONCURRENCY)
        .map(|r| r.unwrap_or(false))
        .collect()
        .await;
    let written = results.iter().filter(|&&ok| ok).count() as u32;
    failed += results.len() as u32 - written;
    Ok(ExportResult {
        written,
        failed,
        dest: dest.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
pub async fn export_entries(
    state: State<'_, Arc<AppState>>,
    ids: Vec<u32>,
    dest: String,
) -> Result<ExportResult, String> {
    export(state.inner().clone(), ids, PathBuf::from(dest)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_names() {
        assert_eq!(
            sanitize("a<b>:c\"d|e?f*.png", "png"),
            ("a_b__c_d_e_f_".into(), "png".into())
        );
        assert_eq!(sanitize("dir/sub\\x.gif", "gif").0, "dir_sub_x");
        assert_eq!(sanitize("trailing. . .png", "png").0, "trailing");
        assert_eq!(sanitize("CON.png", "png").0, "_CON");
        assert_eq!(sanitize("nul.tar.png", "png").0, "_nul.tar");
        assert_eq!(sanitize("console.png", "png").0, "console");
        assert_eq!(sanitize(".png", "png").0, "file");
        assert_eq!(sanitize("tab\there.webp", "webp").0, "tab_here");
        let long = "é".repeat(400) + ".jpg";
        let (b, e) = sanitize(&long, "jpg");
        assert_eq!(b.chars().count() + 1 + e.len(), MAX_NAME_CHARS);
    }

    #[test]
    fn unique_names_avoid_collisions_and_existing_files() {
        let t = tempfile::tempdir().unwrap();
        std::fs::write(t.path().join("cat.png"), b"existing").unwrap();
        let items = vec![
            ("cat.png".to_string(), "png".to_string()),
            ("Cat.png".to_string(), "png".to_string()),
            ("dog.gif".to_string(), "gif".to_string()),
            ("dog.gif".to_string(), "gif".to_string()),
        ];
        assert_eq!(
            assign_names(&items, t.path()),
            vec!["cat (2).png", "Cat (3).png", "dog.gif", "dog (2).gif"]
        );
    }

    #[test]
    fn exports_scanned_entries() {
        let cache = tempfile::tempdir().unwrap();
        crate::cache::blockfile::tests::build_blockfile_dir(cache.path());
        let state = Arc::new(AppState::default());
        let entries = crate::scan::tests::entries_of(&crate::scan::tests::scan_dir(
            state.clone(),
            cache.path(),
        ));
        let out = tempfile::tempdir().unwrap();
        let mut ids: Vec<u32> = entries.iter().map(|e| e.id).collect();
        ids.push(999_999); // unknown id
        let res =
            tauri::async_runtime::block_on(export(state, ids, out.path().to_path_buf())).unwrap();
        assert_eq!((res.written, res.failed), (4, 1));

        let gif = std::fs::read(out.path().join("tenor.gif")).unwrap();
        assert_eq!(gif, crate::cache::blockfile::tests::GIF);
        let emoji_name = entries
            .iter()
            .find(|e| e.kind == crate::classify::Kind::Emoji)
            .unwrap();
        let png = std::fs::read(out.path().join(&emoji_name.file_name)).unwrap();
        assert_eq!(png, crate::cache::blockfile::tests::PNG);
        let mtime = filetime::FileTime::from_last_modification_time(
            &std::fs::metadata(out.path().join("tenor.gif")).unwrap(),
        );
        let e = entries.iter().find(|e| e.file_name == "tenor.gif").unwrap();
        assert_eq!(mtime.unix_seconds(), e.cached_at.unwrap() / 1000);
    }
}
