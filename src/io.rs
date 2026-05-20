//! Filesystem I/O. The only module in the binary that touches disk during
//! export. Everything else (cfg generation, validation, sanitization) is
//! pure and computed against in-memory data.
//!
//! Two non-trivial behaviors live here:
//!
//! * **Atomic writes** — every cfg file is written to a `*.tmp` sibling and
//!   then renamed into place. A crash mid-export can leave at most one stray
//!   `.tmp` file; it never leaves a half-written `bindmgr_page_*.cfg`.
//! * **One-time autoexec backup** — the very first time we modify
//!   `autoexec.cfg`, we copy the original to `autoexec.cfg.bak`. We detect
//!   "first time" by the absence of our marker block; subsequent edits skip
//!   the backup so the user's pristine original survives.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::cfg::{
    generate_page_files, update_autoexec, validate, GeneratedFile, MessageWarning, ValidationError,
    AUTOEXEC_BEGIN, PAGE_FILE_PREFIX,
};
use crate::model::AppConfig;

const AUTOEXEC_FILENAME: &str = "autoexec.cfg";
const AUTOEXEC_BACKUP_FILENAME: &str = "autoexec.cfg.bak";

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("validation failed")]
    Validation(Vec<ValidationError>),

    #[error("the chosen path does not look like a CS2 cfg directory: {0}")]
    InvalidCfgDir(PathBuf),

    #[error("no CS2 cfg directory has been chosen yet")]
    MissingCfgDir,

    #[error("filesystem error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// What was successfully written, plus any non-fatal sanitization warnings.
#[derive(Debug, Default)]
pub struct ExportReport {
    pub written: Vec<PathBuf>,
    pub warnings: Vec<MessageWarning>,
}

/// Validates `cfg`, then writes every generated file plus the autoexec block.
///
/// On success returns the list of files we touched and any sanitization
/// warnings the caller should surface to the user.
pub fn export(cfg: &AppConfig) -> Result<ExportReport, ExportError> {
    if let Err(errors) = validate(cfg) {
        return Err(ExportError::Validation(errors));
    }

    let dir = cfg
        .cs2_cfg_dir
        .as_deref()
        .ok_or(ExportError::MissingCfgDir)?;

    if !looks_like_cs2_cfg_dir(dir) {
        return Err(ExportError::InvalidCfgDir(dir.to_path_buf()));
    }

    let mut report = ExportReport::default();
    let files = generate_page_files(cfg);

    for f in &files {
        report.warnings.extend(f.warnings.iter().cloned());
        let target = dir.join(&f.filename);
        atomic_write(&target, f.contents.as_bytes())?;
        report.written.push(target);
    }

    delete_orphan_pages(dir, &files)?;

    let autoexec_path = dir.join(AUTOEXEC_FILENAME);
    let existing = read_optional(&autoexec_path)?;
    if !existing.contains(AUTOEXEC_BEGIN) && autoexec_path.exists() {
        let bak = dir.join(AUTOEXEC_BACKUP_FILENAME);
        if !bak.exists() {
            fs::copy(&autoexec_path, &bak).map_err(|e| ExportError::Io {
                path: bak.clone(),
                source: e,
            })?;
        }
    }
    let new_contents = update_autoexec(&existing, cfg);
    if new_contents != existing {
        atomic_write(&autoexec_path, new_contents.as_bytes())?;
    }
    report.written.push(autoexec_path);

    Ok(report)
}

/// Returns true if the path looks like a CS2 cfg directory:
///   `…/<something>/csgo/cfg`
///
/// We match the canonical layout exactly. False here means we refuse to
/// write — the cost of a false negative is a confused user, the cost of a
/// false positive is the user's documents folder getting littered with cfg
/// files.
pub fn looks_like_cs2_cfg_dir(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    let last_two: Vec<_> = path
        .iter()
        .rev()
        .take(2)
        .map(|s| s.to_string_lossy().to_lowercase())
        .collect();
    matches!(
        last_two.as_slice(),
        [cfg_seg, csgo_seg] if cfg_seg == "cfg" && csgo_seg == "csgo"
    )
}

/// Best-effort guess at the default CS2 cfg directory on this machine.
/// Returns `Some` only if the path actually exists.
#[cfg(target_os = "windows")]
pub fn detect_default_cfg_dir() -> Option<PathBuf> {
    const CANDIDATES: &[&str] = &[
        r"C:\Program Files (x86)\Steam\steamapps\common\Counter-Strike Global Offensive\game\csgo\cfg",
        r"C:\Program Files\Steam\steamapps\common\Counter-Strike Global Offensive\game\csgo\cfg",
    ];
    CANDIDATES.iter().map(PathBuf::from).find(|p| p.is_dir())
}

#[cfg(not(target_os = "windows"))]
pub fn detect_default_cfg_dir() -> Option<PathBuf> {
    None
}

fn read_optional(path: &Path) -> Result<String, ExportError> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(ExportError::Io {
            path: path.to_path_buf(),
            source: e,
        }),
    }
}

fn atomic_write(target: &Path, bytes: &[u8]) -> Result<(), ExportError> {
    let mut tmp = target.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);

    {
        let mut f = fs::File::create(&tmp).map_err(|e| ExportError::Io {
            path: tmp.clone(),
            source: e,
        })?;
        f.write_all(bytes).map_err(|e| ExportError::Io {
            path: tmp.clone(),
            source: e,
        })?;
        f.sync_all().map_err(|e| ExportError::Io {
            path: tmp.clone(),
            source: e,
        })?;
    }

    fs::rename(&tmp, target).map_err(|e| ExportError::Io {
        path: target.to_path_buf(),
        source: e,
    })
}

/// Delete any `bindmgr_page_*.cfg` files in `dir` that we didn't just write.
/// Without this, shrinking from 5 pages to 3 would leave `bindmgr_page_4.cfg`
/// and `bindmgr_page_5.cfg` orphaned, breaking the cycle if anything `exec`s
/// them.
fn delete_orphan_pages(dir: &Path, kept: &[GeneratedFile]) -> Result<(), ExportError> {
    let kept_names: std::collections::HashSet<&str> =
        kept.iter().map(|f| f.filename.as_str()).collect();

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            return Err(ExportError::Io {
                path: dir.to_path_buf(),
                source: e,
            })
        }
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(PAGE_FILE_PREFIX)
            && name_str.ends_with(".cfg")
            && !kept_names.contains(name_str.as_ref())
        {
            let _ = fs::remove_file(entry.path());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{KeyBind, Page};
    use tempfile::TempDir;

    fn make_cs2_layout() -> (TempDir, PathBuf) {
        let td = TempDir::new().unwrap();
        let cfg_dir = td.path().join("game").join("csgo").join("cfg");
        fs::create_dir_all(&cfg_dir).unwrap();
        (td, cfg_dir)
    }

    fn sample_cfg(cfg_dir: &Path) -> AppConfig {
        let mk = |name: &str, msg: &str| Page {
            binds: vec![KeyBind {
                key: "1".into(),
                message: msg.into(),
            }],
            ..Page::new(name)
        };
        AppConfig {
            cs2_cfg_dir: Some(cfg_dir.to_path_buf()),
            toggle_key: "F1".into(),
            pages: vec![mk("A", "hi"), mk("B", "yo")],
            selected_page: 0,
        }
    }

    #[test]
    fn looks_like_cs2_cfg_dir_accepts_canonical_path() {
        let (_td, cfg_dir) = make_cs2_layout();
        assert!(looks_like_cs2_cfg_dir(&cfg_dir));
    }

    #[test]
    fn looks_like_cs2_cfg_dir_rejects_arbitrary_directory() {
        let td = TempDir::new().unwrap();
        let bad = td.path().join("documents").join("cfg");
        fs::create_dir_all(&bad).unwrap();
        assert!(!looks_like_cs2_cfg_dir(&bad));
    }

    #[test]
    fn looks_like_cs2_cfg_dir_rejects_nonexistent_path() {
        assert!(!looks_like_cs2_cfg_dir(Path::new(
            "/definitely/not/a/real/place/cfg"
        )));
    }

    #[test]
    fn export_writes_pages_and_autoexec() {
        let (_td, cfg_dir) = make_cs2_layout();
        let cfg = sample_cfg(&cfg_dir);

        let report = export(&cfg).unwrap();

        assert!(cfg_dir.join("bindmgr_page_1.cfg").exists());
        assert!(cfg_dir.join("bindmgr_page_2.cfg").exists());
        assert!(cfg_dir.join("autoexec.cfg").exists());
        assert!(report.warnings.is_empty());

        let autoexec = fs::read_to_string(cfg_dir.join("autoexec.cfg")).unwrap();
        assert!(autoexec.contains("exec bindmgr_page_1"));
    }

    #[test]
    fn export_creates_one_time_backup_then_skips() {
        let (_td, cfg_dir) = make_cs2_layout();
        let autoexec_path = cfg_dir.join("autoexec.cfg");
        fs::write(&autoexec_path, "// my pristine settings\nsensitivity 2.0\n").unwrap();
        let cfg = sample_cfg(&cfg_dir);

        export(&cfg).unwrap();
        let bak_path = cfg_dir.join("autoexec.cfg.bak");
        assert!(bak_path.exists());
        assert_eq!(
            fs::read_to_string(&bak_path).unwrap(),
            "// my pristine settings\nsensitivity 2.0\n"
        );

        fs::write(&bak_path, "STAMP").unwrap();
        export(&cfg).unwrap();
        assert_eq!(fs::read_to_string(&bak_path).unwrap(), "STAMP");
    }

    #[test]
    fn export_refuses_paths_that_are_not_cs2_cfg() {
        let td = TempDir::new().unwrap();
        let cfg = AppConfig {
            cs2_cfg_dir: Some(td.path().to_path_buf()),
            pages: vec![Page {
                binds: vec![KeyBind {
                    key: "1".into(),
                    message: "hi".into(),
                }],
                ..Page::new("P")
            }],
            ..AppConfig::default()
        };
        match export(&cfg) {
            Err(ExportError::InvalidCfgDir(_)) => {}
            other => panic!("expected InvalidCfgDir, got {other:?}"),
        }
    }

    #[test]
    fn export_refuses_invalid_app_config() {
        let (_td, cfg_dir) = make_cs2_layout();
        let mut cfg = sample_cfg(&cfg_dir);
        cfg.toggle_key = "hyperspace".into();
        match export(&cfg) {
            Err(ExportError::Validation(_)) => {}
            other => panic!("expected Validation, got {other:?}"),
        }
    }

    #[test]
    fn export_deletes_orphaned_page_files() {
        let (_td, cfg_dir) = make_cs2_layout();
        fs::write(cfg_dir.join("bindmgr_page_4.cfg"), "stale").unwrap();
        fs::write(cfg_dir.join("bindmgr_page_99.cfg"), "stale").unwrap();
        fs::write(cfg_dir.join("user_other.cfg"), "untouched").unwrap();

        let cfg = sample_cfg(&cfg_dir);
        export(&cfg).unwrap();

        assert!(!cfg_dir.join("bindmgr_page_4.cfg").exists());
        assert!(!cfg_dir.join("bindmgr_page_99.cfg").exists());
        assert!(
            cfg_dir.join("user_other.cfg").exists(),
            "non-bindmgr files must be left alone"
        );
    }

    #[test]
    fn second_export_with_no_changes_is_a_no_op_for_autoexec() {
        let (_td, cfg_dir) = make_cs2_layout();
        let cfg = sample_cfg(&cfg_dir);
        export(&cfg).unwrap();
        let after_first = fs::read_to_string(cfg_dir.join("autoexec.cfg")).unwrap();
        export(&cfg).unwrap();
        let after_second = fs::read_to_string(cfg_dir.join("autoexec.cfg")).unwrap();
        assert_eq!(after_first, after_second);
    }
}
