//! IPC types (see docs/ARCHITECTURE.md, "IPC contract") and shared app state.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::RwLock;

use serde::Serialize;

use crate::classify::Kind;
use crate::sources::Source;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: u32,
    pub url: Option<String>,
    pub original_url: Option<String>,
    pub host: Option<String>,
    pub kind: Kind,
    pub file_name: String,
    pub mime: String,
    pub ext: String,
    pub size: u64,
    pub posted_at: Option<i64>,
    pub cached_at: Option<i64>,
    pub channel_id: Option<String>,
    pub guild_id: Option<String>,
    pub group_key: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ScanEvent {
    Started {
        total: u32,
        sources: Vec<Source>,
    },
    Batch {
        entries: Vec<Entry>,
        done: u32,
        total: u32,
    },
    Finished {
        count: u32,
        elapsed_ms: u64,
        errors: u32,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub written: u32,
    pub failed: u32,
    pub dest: String,
}

/// Where an entry's body lives on disk. Private to the backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub path: PathBuf,
    pub offset: u64,
    pub len: u64,
}

#[derive(Debug, Clone)]
pub struct StoredEntry {
    pub entry: Entry,
    pub loc: Location,
}

#[derive(Default)]
pub struct AppState {
    /// Incremented by every scan; workers of older scans stop when it changes.
    pub generation: AtomicU64,
    pub next_id: AtomicU32,
    pub entries: RwLock<HashMap<u32, StoredEntry>>,
}

impl AppState {
    pub fn is_current(&self, generation: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_event_json_shape() {
        let ev = ScanEvent::Finished {
            count: 3,
            elapsed_ms: 12,
            errors: 1,
        };
        assert_eq!(
            serde_json::to_string(&ev).unwrap(),
            r#"{"type":"finished","data":{"count":3,"elapsedMs":12,"errors":1}}"#
        );
        let ev = ScanEvent::Batch {
            entries: vec![],
            done: 1,
            total: 2,
        };
        assert_eq!(
            serde_json::to_string(&ev).unwrap(),
            r#"{"type":"batch","data":{"entries":[],"done":1,"total":2}}"#
        );
    }
}
