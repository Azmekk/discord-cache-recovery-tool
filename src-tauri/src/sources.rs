//! Detection of Discord installs and their cache format.

use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Blockfile,
    Simple,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub label: String,
    pub path: String,
    pub format: Format,
}

const VARIANTS: &[(&str, &str)] = &[
    ("discord", "Discord"),
    ("discordptb", "Discord PTB"),
    ("discordcanary", "Discord Canary"),
    ("discorddevelopment", "Discord Development"),
];

/// Detects the cache format of a `Cache_Data` folder (only lists its directory).
pub fn detect_format(dir: &Path) -> Format {
    if dir.join("data_1").is_file() && dir.join("index").is_file() {
        return Format::Blockfile;
    }
    let simple = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .any(|e| crate::cache::simple::is_entry_file_name(&e.file_name().to_string_lossy()))
        })
        .unwrap_or(false);
    if simple {
        Format::Simple
    } else {
        Format::Unknown
    }
}

pub fn source_for(label: &str, dir: &Path) -> Source {
    Source {
        label: label.to_string(),
        path: dir.to_string_lossy().into_owned(),
        format: detect_format(dir),
    }
}

/// Candidate `(label, config_root)` pairs for this platform. Each root holds `<variant>/`.
fn roots() -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    #[cfg(windows)]
    if let Some(appdata) = std::env::var_os("APPDATA") {
        out.push((String::new(), PathBuf::from(appdata)));
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = std::env::var_os("HOME") {
        out.push((
            String::new(),
            PathBuf::from(home).join("Library/Application Support"),
        ));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| home.as_ref().map(|h| h.join(".config")));
        if let Some(c) = config {
            out.push((String::new(), c));
        }
        if let Some(h) = home {
            out.push((
                " (Flatpak)".into(),
                h.join(".var/app/com.discordapp.Discord/config"),
            ));
            out.push((" (Snap)".into(), h.join("snap/discord/current/.config")));
        }
    }
    out
}

/// Lists existing Discord cache folders. `DCRT_CACHE_DIR` overrides detection.
pub fn list_sources() -> Vec<Source> {
    if let Some(dir) = std::env::var_os("DCRT_CACHE_DIR") {
        let dir = PathBuf::from(dir);
        return if dir.is_dir() {
            vec![source_for("Custom", &dir)]
        } else {
            Vec::new()
        };
    }
    let mut out: Vec<Source> = Vec::new();
    for (suffix, root) in roots() {
        for (variant, label) in VARIANTS {
            let dir = root.join(variant).join("Cache").join("Cache_Data");
            if dir.is_dir() && !out.iter().any(|s| Path::new(&s.path) == dir) {
                out.push(source_for(&format!("{label}{suffix}"), &dir));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_formats() {
        let t = tempfile::tempdir().unwrap();
        assert_eq!(detect_format(t.path()), Format::Unknown);
        std::fs::write(t.path().join("0123456789abcdef_0"), b"x").unwrap();
        assert_eq!(detect_format(t.path()), Format::Simple);
        std::fs::write(t.path().join("index"), b"x").unwrap();
        std::fs::write(t.path().join("data_1"), b"x").unwrap();
        assert_eq!(detect_format(t.path()), Format::Blockfile);
    }

    #[test]
    fn serializes_camel_case() {
        let s = Source {
            label: "Custom".into(),
            path: "/x".into(),
            format: Format::Blockfile,
        };
        assert_eq!(
            serde_json::to_string(&s).unwrap(),
            r#"{"label":"Custom","path":"/x","format":"blockfile"}"#
        );
    }
}
