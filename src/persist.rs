//! App-state persistence to `%APPDATA%\cs2-keybind-manager\state.json`.
//!
//! Loading is forgiving: a missing file (first launch) yields a default
//! config; a corrupted file is logged and also yields a default rather than
//! crashing. The user's pages should never be silently destroyed, so we save
//! atomically (write to a `.tmp` sibling and rename).

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use directories::ProjectDirs;
use thiserror::Error;

use crate::model::AppConfig;

const APP_QUALIFIER: &str = "";
const APP_ORG: &str = "";
const APP_NAME: &str = "cs2-keybind-manager";
const STATE_FILENAME: &str = "state.json";

#[derive(Debug, Error)]
pub enum PersistError {
    #[error("could not resolve a config directory for this user")]
    NoConfigDir,
    #[error("filesystem error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not serialize state: {0}")]
    Serialize(#[from] serde_json::Error),
}

fn state_path() -> Result<PathBuf, PersistError> {
    let dirs =
        ProjectDirs::from(APP_QUALIFIER, APP_ORG, APP_NAME).ok_or(PersistError::NoConfigDir)?;
    Ok(dirs.config_dir().join(STATE_FILENAME))
}

/// Loads the saved app state. Returns `AppConfig::default()` on any
/// recoverable issue (missing file, parse failure) and logs a warning so the
/// user notices.
pub fn load() -> AppConfig {
    let path = match state_path() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("could not locate state file: {e}; starting fresh");
            return AppConfig::default();
        }
    };

    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return AppConfig::default(),
        Err(e) => {
            tracing::warn!("failed to read {}: {e}; starting fresh", path.display());
            return AppConfig::default();
        }
    };

    match serde_json::from_str(&raw) {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::warn!(
                "failed to parse {}: {e}; starting fresh (existing file left untouched)",
                path.display()
            );
            AppConfig::default()
        }
    }
}

/// Persists `cfg` to disk atomically.
pub fn save(cfg: &AppConfig) -> Result<(), PersistError> {
    let path = state_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| PersistError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    let json = serde_json::to_string_pretty(cfg)?;
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);

    {
        let mut f = fs::File::create(&tmp).map_err(|e| PersistError::Io {
            path: tmp.clone(),
            source: e,
        })?;
        f.write_all(json.as_bytes()).map_err(|e| PersistError::Io {
            path: tmp.clone(),
            source: e,
        })?;
        f.sync_all().map_err(|e| PersistError::Io {
            path: tmp.clone(),
            source: e,
        })?;
    }

    fs::rename(&tmp, &path).map_err(|e| PersistError::Io { path, source: e })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{KeyBind, Page};

    /// Round-trip through the JSON serializer directly. We don't exercise the
    /// real `state_path()` from inside tests because it depends on user-level
    /// directories the test environment shouldn't touch.
    #[test]
    fn round_trip_preserves_pages() {
        let cfg = AppConfig {
            cs2_cfg_dir: Some(PathBuf::from("C:/cs2/cfg")),
            toggle_key: "F2".into(),
            pages: vec![Page {
                name: "Trash".into(),
                binds: vec![KeyBind {
                    key: "1".into(),
                    message: "ez".into(),
                }],
            }],
            selected_page: 0,
        };
        let json = serde_json::to_string_pretty(&cfg).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.pages.len(), 1);
        assert_eq!(back.pages[0].binds[0].message, "ez");
    }
}
