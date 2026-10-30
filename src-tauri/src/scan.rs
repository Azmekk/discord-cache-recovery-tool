//! The scan pipeline: list → parse metadata → per-entry jobs on the blocking pool →
//! batched results over a `Channel`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant, UNIX_EPOCH};

use futures::StreamExt;
use tauri::ipc::Channel;
use tauri::State;

use crate::cache::{blockfile, simple};
use crate::classify::classify;
use crate::io::{open_ro, read_vec};
use crate::model::{AppState, Entry, Location, ScanEvent, StoredEntry};
use crate::sniff::{from_content_type, sniff, SNIFF_LEN};
use crate::sources::{list_sources, source_for, Source};

const CONCURRENCY: usize = 64;
const BATCH_SIZE: usize = 200;
const BATCH_INTERVAL: Duration = Duration::from_millis(80);

enum Job {
    Block(blockfile::BlockEntry),
    Simple(PathBuf),
    Orphan(PathBuf),
}

impl Job {
    /// Index into the scan-details tally.
    fn kind(&self) -> usize {
        match self {
            Job::Block(_) => 0,
            Job::Simple(_) => 1,
            Job::Orphan(_) => 2,
        }
    }
}

struct SourceJob {
    job: Job,
    source: Arc<str>,
}

/// An entry before it gets an id.
struct Found {
    entry: Entry,
    loc: Location,
}

enum Outcome {
    Found(Box<Found>),
    Skipped,
    Error,
}

#[tauri::command]
pub async fn scan(
    state: State<'_, Arc<AppState>>,
    dirs: Option<Vec<String>>,
    on_event: Channel<ScanEvent>,
) -> Result<(), String> {
    let state = state.inner().clone();
    let sources = tauri::async_runtime::spawn_blocking(move || resolve_sources(dirs))
        .await
        .map_err(|e| e.to_string())?;
    run_scan(state, sources, move |ev| {
        let _ = on_event.send(ev);
    })
    .await;
    Ok(())
}

fn resolve_sources(dirs: Option<Vec<String>>) -> Vec<Source> {
    let detected = list_sources();
    match dirs {
        None => detected,
        Some(dirs) => dirs
            .into_iter()
            .map(PathBuf::from)
            .filter(|d| d.is_dir())
            .map(|d| {
                let label = detected
                    .iter()
                    .find(|s| Path::new(&s.path) == d)
                    .map(|s| s.label.clone())
                    .unwrap_or_else(|| "Custom".to_string());
                source_for(&label, &d)
            })
            .collect(),
    }
}

/// Runs a scan over `sources`, emitting events via `emit`. Cancels any running scan.
pub async fn run_scan<F>(state: Arc<AppState>, sources: Vec<Source>, emit: F)
where
    F: Fn(ScanEvent) + Send + Sync + 'static,
{
    let started = Instant::now();
    let generation = {
        let mut map = state.entries.write().unwrap_or_else(|e| e.into_inner());
        map.clear();
        state.generation.fetch_add(1, Ordering::SeqCst) + 1
    };

    // Phase 1: list directories and parse blockfile metadata.
    let st = state.clone();
    let srcs = sources.clone();
    let (jobs, reports) = tauri::async_runtime::spawn_blocking(move || {
        let cancelled = || !st.is_current(generation);
        let mut jobs = Vec::new();
        let mut reports = Vec::new();
        for s in &srcs {
            if cancelled() {
                break;
            }
            reports.push(collect_jobs(Path::new(&s.path), s, &cancelled, &mut jobs));
        }
        (jobs, reports)
    })
    .await
    .unwrap_or_default();
    let mut errors: u32 = reports.iter().map(|r| r.errors).sum();

    if !state.is_current(generation) {
        return;
    }
    let total = jobs.len() as u32;
    emit(ScanEvent::Started { total, sources });

    // Phase 2: per-entry jobs on the blocking pool.
    let mut results = futures::stream::iter(jobs)
        .map(|job| {
            let st = state.clone();
            let kind = job.job.kind();
            tauri::async_runtime::spawn_blocking(move || {
                if !st.is_current(generation) {
                    return (kind, Outcome::Skipped);
                }
                (kind, run_job(job))
            })
        })
        .buffer_unordered(CONCURRENCY);

    let mut done = 0u32;
    let mut count = 0u32;
    // Found / skipped (not media) per job kind, for the scan details.
    let mut tally = [[0u32; 2]; 3];
    let mut batch: Vec<Entry> = Vec::new();
    let mut last_flush = Instant::now();
    while let Some(res) = results.next().await {
        if !state.is_current(generation) {
            return;
        }
        done += 1;
        match res {
            Ok((kind, Outcome::Found(f))) => {
                tally[kind][0] += 1;
                let id = state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
                let mut entry = f.entry;
                entry.id = id;
                {
                    let mut map = state.entries.write().unwrap_or_else(|e| e.into_inner());
                    if !state.is_current(generation) {
                        return;
                    }
                    map.insert(
                        id,
                        StoredEntry {
                            entry: entry.clone(),
                            loc: f.loc,
                        },
                    );
                }
                batch.push(entry);
                count += 1;
            }
            Ok((kind, Outcome::Skipped)) => tally[kind][1] += 1,
            Ok((_, Outcome::Error)) | Err(_) => errors += 1,
        }
        if batch.len() >= BATCH_SIZE || last_flush.elapsed() >= BATCH_INTERVAL {
            emit(ScanEvent::Batch {
                entries: std::mem::take(&mut batch),
                done,
                total,
            });
            last_flush = Instant::now();
        }
    }
    if !state.is_current(generation) {
        return;
    }
    if !batch.is_empty() || done > 0 {
        emit(ScanEvent::Batch {
            entries: batch,
            done,
            total,
        });
    }
    let elapsed_ms = started.elapsed().as_millis() as u64;
    let mut diagnostics = format!(
        "Discord Cache Recovery {} ({})\nScan: {count} files in {elapsed_ms} ms, {errors} errors\n\
         Cache entries: {} media, {} not media\nSimple-cache entries: {} media, {} not media\n\
         Loose files: {} media, {} not media\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        tally[0][0],
        tally[0][1],
        tally[1][0],
        tally[1][1],
        tally[2][0],
        tally[2][1],
    );
    for r in &reports {
        diagnostics.push('\n');
        diagnostics.push_str(&r.details);
    }
    emit(ScanEvent::Finished {
        count,
        elapsed_ms,
        errors,
        locked_sources: reports
            .iter()
            .filter(|r| r.locked)
            .map(|r| r.label.clone())
            .collect(),
        diagnostics,
    });
}

/// What phase 1 learned about one source.
struct SourceReport {
    label: String,
    errors: u32,
    /// Discord held the cache index locked, so names, links and dates are unavailable.
    locked: bool,
    /// Counts only, for the scan details.
    details: String,
}

/// Files in a cache folder that are never media bodies.
fn is_metadata_name(name: &str) -> bool {
    name == "index"
        || name == "index-dir"
        || name.starts_with("data_")
        || name.ends_with("_1")
        || name.ends_with("_s")
        || name.starts_with("todelete_")
}

/// Lists `dir` and appends its jobs.
fn collect_jobs(
    dir: &Path,
    src: &Source,
    cancelled: &dyn Fn() -> bool,
    jobs: &mut Vec<SourceJob>,
) -> SourceReport {
    let source: Arc<str> = Arc::from(src.label.as_str());
    let mut report = SourceReport {
        label: src.label.clone(),
        errors: 0,
        locked: false,
        details: format!("{} ({:?} format)\n", src.label, src.format),
    };
    let Ok(rd) = std::fs::read_dir(dir) else {
        report.errors = 1;
        report.details.push_str("  folder could not be listed\n");
        return report;
    };
    let mut files: Vec<(String, PathBuf)> = rd
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
        .collect();
    files.sort();

    report.details.push_str(&format!("  files in folder: {}\n", files.len()));

    let mut referenced = HashSet::new();
    if dir.join("data_1").is_file() {
        match blockfile::parse_dir(dir, cancelled) {
            Ok(scan) => {
                let s = &scan.stats;
                report.locked = s.locked > 0;
                report.details.push_str(&format!(
                    "  data files: {} found, {} entry files, {} locked, {} unreadable\n  \
                     allocated blocks: {}\n  entries accepted: {}, body missing: {}\n  \
                     rejected blocks: state {}, key length {}, time {}, body address {}, \
                     body size {}, key {}\n",
                    s.data_files,
                    s.entry_files,
                    s.locked,
                    s.unreadable,
                    s.blocks_allocated,
                    s.accepted,
                    s.body_missing,
                    s.rej_state,
                    s.rej_key_len,
                    s.rej_time,
                    s.rej_body_addr,
                    s.rej_body_size,
                    s.rej_key,
                ));
                report.errors += scan.errors;
                referenced = scan.referenced;
                jobs.extend(scan.entries.into_iter().map(|e| SourceJob {
                    job: Job::Block(e),
                    source: source.clone(),
                }));
            }
            Err(_) => report.errors += 1,
        }
    }
    let (mut simple_n, mut loose_n) = (0, 0);
    for (name, path) in files {
        if simple::is_entry_file_name(&name) {
            simple_n += 1;
            jobs.push(SourceJob {
                job: Job::Simple(path),
                source: source.clone(),
            });
        } else if !is_metadata_name(&name) && !referenced.contains(&name) {
            loose_n += 1;
            jobs.push(SourceJob {
                job: Job::Orphan(path),
                source: source.clone(),
            });
        }
    }
    report.details.push_str(&format!(
        "  simple-cache files: {simple_n}, loose files: {loose_n}\n"
    ));
    report
}

fn mtime_ms(path: &Path) -> Option<i64> {
    let m = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(m.duration_since(UNIX_EPOCH).ok()?.as_millis() as i64)
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

struct Raw<'a> {
    key: Option<&'a str>,
    head: &'a [u8],
    content_type: Option<&'a str>,
    cached_at: Option<i64>,
    loc: Location,
    source: &'a str,
}

fn build(raw: Raw) -> Outcome {
    if raw.loc.len == 0 {
        return Outcome::Skipped;
    }
    let Some(media) = sniff(raw.head).or_else(|| raw.content_type.and_then(from_content_type))
    else {
        return Outcome::Skipped;
    };
    let cache_file = file_name_of(&raw.loc.path);
    let orphan_key = format!("orphan:{}:{}", raw.loc.path.display(), raw.loc.offset);
    let c = classify(raw.key, &media, &cache_file, &orphan_key);
    Outcome::Found(Box::new(Found {
        entry: Entry {
            id: 0,
            url: c.url,
            original_url: c.original_url,
            host: c.host,
            kind: c.kind,
            file_name: c.file_name,
            mime: media.mime.to_string(),
            ext: media.ext.to_string(),
            size: raw.loc.len,
            posted_at: c.posted_at,
            cached_at: raw.cached_at,
            channel_id: c.channel_id,
            guild_id: c.guild_id,
            group_key: c.group_key,
            source: raw.source.to_string(),
        },
        loc: raw.loc,
    }))
}

fn run_job(sj: SourceJob) -> Outcome {
    let source = &*sj.source;
    match sj.job {
        Job::Block(e) => {
            let n = e.body_len.min(SNIFF_LEN as u64) as usize;
            let head = match &e.body_handle {
                Some(h) => read_vec(h, e.body_offset, n),
                None => open_ro(&e.body_path).and_then(|f| read_vec(&f, e.body_offset, n)),
            };
            let Ok(head) = head else {
                return Outcome::Error;
            };
            let cached_at = e
                .info
                .response_time
                .or(e.creation_time)
                .or(e.info.date)
                .or_else(|| mtime_ms(&e.body_path));
            build(Raw {
                key: Some(&e.key),
                head: &head,
                content_type: e.info.content_type.as_deref(),
                cached_at,
                loc: Location {
                    path: e.body_path.clone(),
                    offset: e.body_offset,
                    len: e.body_len,
                },
                source,
            })
        }
        Job::Simple(path) => {
            let Ok(s) = simple::parse_path(&path) else {
                return Outcome::Error;
            };
            let cached_at = s
                .info
                .response_time
                .or(s.info.date)
                .or_else(|| mtime_ms(&path));
            build(Raw {
                key: Some(&s.key),
                head: &s.head,
                content_type: s.info.content_type.as_deref(),
                cached_at,
                loc: Location {
                    path,
                    offset: s.body_offset,
                    len: s.body_len,
                },
                source,
            })
        }
        Job::Orphan(path) => {
            let res = open_ro(&path).and_then(|f| {
                let len = f.metadata()?.len();
                Ok((len, read_vec(&f, 0, SNIFF_LEN)?))
            });
            let Ok((len, head)) = res else {
                return Outcome::Error;
            };
            build(Raw {
                key: None,
                head: &head,
                content_type: None,
                cached_at: mtime_ms(&path),
                loc: Location {
                    path,
                    offset: 0,
                    len,
                },
                source,
            })
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::cache::blockfile::tests as bf;
    use crate::cache::simple::tests as sm;
    use crate::classify::Kind;
    use std::sync::Mutex;

    pub fn scan_dir(state: Arc<AppState>, dir: &Path) -> Vec<ScanEvent> {
        let events = Arc::new(Mutex::new(Vec::new()));
        let ev2 = events.clone();
        let src = source_for("Custom", dir);
        tauri::async_runtime::block_on(run_scan(state, vec![src], move |e| {
            ev2.lock().unwrap().push(e)
        }));
        let v = events.lock().unwrap().clone();
        v
    }

    pub fn entries_of(events: &[ScanEvent]) -> Vec<Entry> {
        let mut out: Vec<Entry> = events
            .iter()
            .flat_map(|e| match e {
                ScanEvent::Batch { entries, .. } => entries.clone(),
                _ => vec![],
            })
            .collect();
        out.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        out
    }

    #[test]
    fn scans_blockfile_dir_end_to_end() {
        let tmp = tempfile::tempdir().unwrap();
        let resp_ms = bf::build_blockfile_dir(tmp.path());
        let state = Arc::new(AppState::default());
        let events = scan_dir(state.clone(), tmp.path());

        assert!(matches!(&events[0], ScanEvent::Started { total: 4, .. }));
        let ScanEvent::Finished { count, errors, .. } = events.last().unwrap() else {
            panic!("no finished event");
        };
        assert_eq!((*count, *errors), (4, 0));

        let entries = entries_of(&events);
        let names: Vec<&str> = entries.iter().map(|e| e.file_name.as_str()).collect();
        assert_eq!(names.len(), 4, "{names:?}");
        let gif = entries.iter().find(|e| e.file_name == "tenor.gif").unwrap();
        assert_eq!(gif.kind, Kind::Gif);
        assert_eq!(gif.cached_at, Some(resp_ms));
        assert_eq!(gif.mime, "image/gif");
        let emoji = entries.iter().find(|e| e.kind == Kind::Emoji).unwrap();
        assert_eq!(emoji.cached_at, Some(bf::CREATED + 1000)); // creation_time fallback
        assert_eq!(emoji.size, bf::PNG.len() as u64);
        let orphan = entries.iter().find(|e| e.kind == Kind::Unknown).unwrap();
        assert_eq!(orphan.file_name, "f_000002.png");
        assert!(entries
            .iter()
            .any(|e| e.kind == Kind::Attachment && e.ext == "gif"));
        assert_eq!(state.entries.read().unwrap().len(), 4);

        let ScanEvent::Finished {
            locked_sources,
            diagnostics,
            ..
        } = events.last().unwrap()
        else {
            unreachable!()
        };
        assert!(locked_sources.is_empty());
        assert!(diagnostics.contains("0 locked"), "{diagnostics}");
        assert!(diagnostics.contains("entries accepted: 3"), "{diagnostics}");
        // Counts only: no keys or URLs leak into the report.
        assert!(!diagnostics.contains("http"), "{diagnostics}");
    }

    /// Discord holds its index files open exclusively while it runs. The scan must report
    /// that instead of silently returning every file as unknown.
    #[cfg(windows)]
    #[test]
    fn reports_locked_cache() {
        use std::os::windows::fs::OpenOptionsExt;
        let tmp = tempfile::tempdir().unwrap();
        bf::build_blockfile_dir(tmp.path());
        let _lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(tmp.path().join("data_1"))
            .unwrap();
        let events = scan_dir(Arc::new(AppState::default()), tmp.path());
        let ScanEvent::Finished {
            locked_sources,
            diagnostics,
            ..
        } = events.last().unwrap()
        else {
            panic!("no finished event");
        };
        assert_eq!(locked_sources, &vec!["Custom".to_string()]);
        assert!(diagnostics.contains("1 locked"), "{diagnostics}");
    }

    #[test]
    fn scans_simple_dir_and_rescan_clears() {
        let tmp = tempfile::tempdir().unwrap();
        let s0 = crate::http::tests::build_pickle(
            1_710_000_000_000,
            &["HTTP/1.1 200", "content-type: image/png"],
        );
        let key = "1/0/_dk_https://discord.com https://discord.com https://cdn.discordapp.com/attachments/111111111111111111/175928847299117063/cat.png?ex=1";
        std::fs::write(
            tmp.path().join("0123456789abcdef_0"),
            sm::build_simple(key, sm::PNG, &s0, true),
        )
        .unwrap();
        // Non-media entry is skipped; zero-length body is skipped.
        std::fs::write(
            tmp.path().join("1123456789abcdef_0"),
            sm::build_simple(
                "https://discord.com/api/v9/x",
                b"{\"a\":1}",
                &crate::http::tests::build_pickle(
                    1_710_000_000_000,
                    &["HTTP/1.1 200", "content-type: application/json"],
                ),
                false,
            ),
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("2123456789abcdef_0"),
            sm::build_simple("https://cdn.discordapp.com/emojis/1.png", b"", &s0, false),
        )
        .unwrap();
        std::fs::write(tmp.path().join("3123456789abcdef_1"), b"stream2").unwrap();
        std::fs::create_dir(tmp.path().join("index-dir")).unwrap();

        let state = Arc::new(AppState::default());
        let entries = entries_of(&scan_dir(state.clone(), tmp.path()));
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.file_name, "cat.png");
        assert_eq!(e.cached_at, Some(1_710_000_000_000));
        assert_eq!(e.posted_at, Some(1_462_015_105_796));
        let first_id = e.id;

        let again = entries_of(&scan_dir(state.clone(), tmp.path()));
        assert_eq!(again.len(), 1);
        assert!(again[0].id > first_id, "ids keep increasing across scans");
        let map = state.entries.read().unwrap();
        assert_eq!(map.len(), 1);
        assert!(map.contains_key(&again[0].id));
    }

    #[test]
    fn superseded_scan_stops_emitting() {
        let tmp = tempfile::tempdir().unwrap();
        bf::build_blockfile_dir(tmp.path());
        let state = Arc::new(AppState::default());
        let events = Arc::new(Mutex::new(Vec::new()));
        let (ev2, st2) = (events.clone(), state.clone());
        let src = source_for("Custom", tmp.path());
        tauri::async_runtime::block_on(run_scan(state.clone(), vec![src], move |e| {
            if matches!(e, ScanEvent::Started { .. }) {
                // Simulate a newer scan starting right now.
                st2.generation.fetch_add(1, Ordering::SeqCst);
            }
            ev2.lock().unwrap().push(e);
        }));
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 1, "only `started` may be emitted");
        assert!(state.entries.read().unwrap().is_empty());
    }
}
