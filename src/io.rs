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

use crate::cfg::import::{parse_keyvalues_bindings, ParsedBind};
use crate::cfg::{
    generate_page_files, update_autoexec, validate, GeneratedFile, MessageWarning, ValidationError,
    AUTOEXEC_BEGIN, PAGE_FILE_PREFIX,
};
use crate::model::AppConfig;

/// File-name prefix for Valve's per-account keybind files. The full name is
/// `cs2_user_keys_<index>_slot<index>.vcfg` (for example
/// `cs2_user_keys_0_slot0.vcfg`). Despite the `.vcfg` extension, the file
/// is plain text: a list of `bind "key" "command"` lines.
const USER_KEYS_PREFIX: &str = "cs2_user_keys_";
const USER_KEYS_SUFFIX: &str = ".vcfg";

/// CS2's Steam App ID. Used to locate the per-account cfg directory inside
/// `Steam/userdata/<account>/<app_id>/local/cfg/`.
const CS2_APP_ID: &str = "730";

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

/// One Valve user-keys file we managed to read, parsed into ready-to-import
/// bind rows.
#[derive(Debug, Clone)]
pub struct ImportedSource {
    /// Full path to the source file. The import dialog surfaces this as a
    /// hover tooltip so users can locate the file in Explorer if curious.
    pub path: PathBuf,
    /// Plain filename for display in the import dialog.
    pub display_name: String,
    pub binds: Vec<ParsedBind>,
}

/// Aggregate result of `scan_existing_binds`. Empty `sources` is the normal
/// "nothing found" state — not an error.
#[derive(Debug, Default, Clone)]
pub struct ImportScan {
    pub sources: Vec<ImportedSource>,
}

impl ImportScan {
    pub fn total_binds(&self) -> usize {
        self.sources.iter().map(|s| s.binds.len()).sum()
    }
}

/// Walks up from a known CS2 cfg directory to discover Valve's per-account
/// keybind files in Steam's userdata tree.
///
/// CS2 stores user-authored binds at
/// `Steam/userdata/<account_id>/730/local/cfg/cs2_user_keys_*_slot*.vcfg`.
/// We derive the Steam root by finding `steamapps` in the cfg dir's
/// ancestors (its parent is the Steam root, and `userdata` is its sibling).
///
/// **Surfaces binds from every Steam account on this machine**, not just
/// the most-recently-active one — users with multiple accounts probably
/// want to pick from all of them. Results are sorted so the output is
/// deterministic.
///
/// Returns an empty `Vec` if Steam root, the userdata dir, or any matching
/// file can't be found. This is the normal "no binds to import" outcome.
pub fn detect_steam_userdata_keybind_files(cfg_dir: &Path) -> Vec<PathBuf> {
    let Some(steam_root) = steam_root_from_cfg_dir(cfg_dir) else {
        return Vec::new();
    };
    let userdata = steam_root.join("userdata");
    if !userdata.is_dir() {
        return Vec::new();
    }

    let mut out = Vec::new();
    let Ok(account_entries) = fs::read_dir(&userdata) else {
        return Vec::new();
    };
    for account in account_entries.flatten() {
        let account_dir = account.path();
        if !account_dir.is_dir() {
            continue;
        }
        let cs2_cfg_dir = account_dir.join(CS2_APP_ID).join("local").join("cfg");
        let Ok(entries) = fs::read_dir(&cs2_cfg_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if is_user_keys_filename(&entry.file_name().to_string_lossy()) {
                out.push(entry.path());
            }
        }
    }
    out.sort();
    out
}

/// Reads every Valve user-keys file we can find and parses it into
/// importable bind rows.
///
/// **Files that contain zero importable binds are dropped** — Steam
/// pre-creates `slot1`/`slot2`/`slot3` files for unused loadout slots,
/// and surfacing those as empty headers in the import dialog is pure
/// noise. Per-file I/O errors are logged and the file is skipped; we
/// never fail the whole scan because of one bad file. An empty return
/// means "nothing importable found", which the UI handles by either
/// suppressing the first-run dialog or showing the "no binds found"
/// toast for a manual scan.
pub fn scan_existing_binds(cfg_dir: &Path) -> ImportScan {
    let files = detect_steam_userdata_keybind_files(cfg_dir);
    let mut sources = Vec::with_capacity(files.len());
    for path in files {
        let display_name = display_name_for(&path);
        match fs::read_to_string(&path) {
            Ok(contents) => {
                let binds = parse_keyvalues_bindings(&contents);
                if binds.is_empty() {
                    continue;
                }
                sources.push(ImportedSource {
                    path,
                    display_name,
                    binds,
                });
            }
            Err(e) => {
                tracing::warn!("could not read {}: {e}", path.display());
            }
        }
    }
    ImportScan { sources }
}

/// Constructs a short, human-readable label for a discovered keys file.
/// Includes the Steam account-id folder so users with multiple accounts
/// can tell sources apart at a glance: `12345 / cs2_user_keys_0_slot0.vcfg`.
fn display_name_for(path: &Path) -> String {
    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    // Walk up: cfg → local → 730 → <accountid>.
    let account = path
        .ancestors()
        .nth(4)
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned());
    match account {
        Some(id) => format!("account {id} / {filename}"),
        None => filename,
    }
}

fn is_user_keys_filename(name: &str) -> bool {
    name.starts_with(USER_KEYS_PREFIX) && name.ends_with(USER_KEYS_SUFFIX)
}

/// Best-effort Steam root from a CS2 cfg directory.
/// `…/Steam/steamapps/common/.../game/csgo/cfg` → `…/Steam`.
fn steam_root_from_cfg_dir(cfg_dir: &Path) -> Option<PathBuf> {
    for ancestor in cfg_dir.ancestors() {
        if ancestor
            .file_name()
            .map(|n| n.eq_ignore_ascii_case("steamapps"))
            .unwrap_or(false)
        {
            return ancestor.parent().map(Path::to_path_buf);
        }
    }
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
                ..KeyBind::default()
            }],
            ..Page::new(name)
        };
        AppConfig {
            cs2_cfg_dir: Some(cfg_dir.to_path_buf()),
            toggle_key: "F1".into(),
            pages: vec![mk("A", "hi"), mk("B", "yo")],
            ..AppConfig::default()
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
                    ..KeyBind::default()
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

    // ────────────────── Import-scan tests ──────────────────
    //
    // These exercise the userdata-tree walk. We simulate the full Steam
    // layout (cfg_dir reachable via a `steamapps` ancestor) so
    // `steam_root_from_cfg_dir` resolves; the userdata subtree is then
    // populated per test.

    /// `<td>/Steam/steamapps/common/Counter-Strike Global Offensive/game/csgo/cfg`,
    /// returns `(td, cfg_dir, steam_root)`.
    fn make_full_steam_layout() -> (TempDir, PathBuf, PathBuf) {
        let td = TempDir::new().unwrap();
        let steam_root = td.path().join("Steam");
        let cfg_dir = steam_root
            .join("steamapps")
            .join("common")
            .join("Counter-Strike Global Offensive")
            .join("game")
            .join("csgo")
            .join("cfg");
        fs::create_dir_all(&cfg_dir).unwrap();
        (td, cfg_dir, steam_root)
    }

    fn write_user_keys(steam_root: &Path, account_id: &str, slot: u32, body: &str) -> PathBuf {
        let dir = steam_root
            .join("userdata")
            .join(account_id)
            .join(CS2_APP_ID)
            .join("local")
            .join("cfg");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("cs2_user_keys_0_slot{slot}.vcfg"));
        fs::write(&path, body).unwrap();
        path
    }

    /// Wrap `pairs` (`(key, value)` strings) into a valid `"bindings"`
    /// section so tests can focus on what's being parsed, not the syntax.
    fn vcfg_with_bindings(pairs: &[(&str, &str)]) -> String {
        let mut s = String::from("\"config\"\n{\n\t\"bindings\"\n\t{\n");
        for (k, v) in pairs {
            s.push_str(&format!("\t\t\"{k}\"\t\t\"{v}\"\n"));
        }
        s.push_str("\t}\n}\n");
        s
    }

    #[test]
    fn detect_returns_empty_when_no_steamapps_ancestor() {
        let td = TempDir::new().unwrap();
        let bad = td.path().join("not").join("a").join("steam").join("dir");
        fs::create_dir_all(&bad).unwrap();
        assert!(detect_steam_userdata_keybind_files(&bad).is_empty());
    }

    #[test]
    fn detect_returns_empty_when_no_userdata_exists() {
        let (_td, cfg_dir, _steam_root) = make_full_steam_layout();
        assert!(detect_steam_userdata_keybind_files(&cfg_dir).is_empty());
    }

    #[test]
    fn detect_finds_single_account_keys_file() {
        let (_td, cfg_dir, steam_root) = make_full_steam_layout();
        let body = vcfg_with_bindings(&[("1", "say hi")]);
        let written = write_user_keys(&steam_root, "12345", 0, &body);

        let found = detect_steam_userdata_keybind_files(&cfg_dir);
        assert_eq!(found, vec![written]);
    }

    #[test]
    fn detect_surfaces_keys_from_every_userdata_account() {
        // Multiple Steam accounts on the same machine should all appear in
        // the scan — the user explicitly asked for this: "offer all keybinds
        // from every user".
        let (_td, cfg_dir, steam_root) = make_full_steam_layout();
        let body = vcfg_with_bindings(&[("1", "say hello")]);
        let a = write_user_keys(&steam_root, "11111", 0, &body);
        let b = write_user_keys(&steam_root, "22222", 0, &body);

        let found = detect_steam_userdata_keybind_files(&cfg_dir);
        assert_eq!(found.len(), 2, "both accounts must be visible");
        assert!(found.contains(&a));
        assert!(found.contains(&b));
    }

    #[test]
    fn detect_returns_all_slot_files_per_account() {
        let (_td, cfg_dir, steam_root) = make_full_steam_layout();
        let body = vcfg_with_bindings(&[("1", "say a")]);
        write_user_keys(&steam_root, "99999", 0, &body);
        write_user_keys(&steam_root, "99999", 1, &body);

        let found = detect_steam_userdata_keybind_files(&cfg_dir);
        assert_eq!(found.len(), 2);
        let names: Vec<String> = found
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(names.iter().any(|n| n == "cs2_user_keys_0_slot0.vcfg"));
        assert!(names.iter().any(|n| n == "cs2_user_keys_0_slot1.vcfg"));
    }

    #[test]
    fn scan_parses_keyvalues_bindings_from_discovered_files() {
        let (_td, cfg_dir, steam_root) = make_full_steam_layout();
        let body =
            vcfg_with_bindings(&[("1", "say Rush B"), ("f5", "slot1"), ("[", "say bracket")]);
        write_user_keys(&steam_root, "12345", 0, &body);

        let scan = scan_existing_binds(&cfg_dir);
        assert_eq!(scan.sources.len(), 1);
        assert_eq!(scan.total_binds(), 3);
        // Display name includes account id so multi-account scans aren't
        // ambiguous.
        assert!(scan.sources[0].display_name.contains("account 12345"));
        assert!(scan.sources[0]
            .display_name
            .contains("cs2_user_keys_0_slot0.vcfg"));
        // Verify a representative bind from each kind. Symbol keys stay
        // in raw form (`[`, not `leftbracket`) — that's the canonical
        // CS2 representation now.
        let keys: Vec<&str> = scan.sources[0]
            .binds
            .iter()
            .map(|b| b.key.as_str())
            .collect();
        assert_eq!(keys, vec!["1", "f5", "["]);
        assert_eq!(scan.sources[0].binds[0].message, "Rush B");
        assert_eq!(scan.sources[0].binds[1].message, "slot1");
        assert_eq!(scan.sources[0].binds[2].message, "bracket");
    }

    #[test]
    fn scan_aggregates_binds_from_multiple_accounts() {
        let (_td, cfg_dir, steam_root) = make_full_steam_layout();
        write_user_keys(
            &steam_root,
            "11111",
            0,
            &vcfg_with_bindings(&[("1", "say from-alice")]),
        );
        write_user_keys(
            &steam_root,
            "22222",
            0,
            &vcfg_with_bindings(&[("2", "say from-bob"), ("3", "say also-bob")]),
        );

        let scan = scan_existing_binds(&cfg_dir);
        assert_eq!(scan.sources.len(), 2);
        assert_eq!(scan.total_binds(), 3);
        // Sources should be addressable by account id via display_name.
        let labels: Vec<&str> = scan
            .sources
            .iter()
            .map(|s| s.display_name.as_str())
            .collect();
        assert!(labels.iter().any(|l| l.contains("account 11111")));
        assert!(labels.iter().any(|l| l.contains("account 22222")));
    }

    #[test]
    fn scan_returns_empty_scan_when_nothing_to_find() {
        let (_td, cfg_dir, _steam_root) = make_full_steam_layout();
        let scan = scan_existing_binds(&cfg_dir);
        assert!(scan.sources.is_empty());
        assert_eq!(scan.total_binds(), 0);
    }

    #[test]
    fn scan_drops_files_that_have_no_importable_binds() {
        // Steam pre-creates slot1/slot2/slot3 vcfg files with an empty
        // `"bindings" {}` block for loadout slots the user never used.
        // Those would show up as dead headers in the import dialog if we
        // didn't filter them out here.
        let (_td, cfg_dir, steam_root) = make_full_steam_layout();
        let empty = "\"config\"\n{\n\t\"bindings\"\n\t{\n\t}\n}\n";
        write_user_keys(&steam_root, "12345", 1, empty);
        write_user_keys(&steam_root, "12345", 2, empty);
        write_user_keys(
            &steam_root,
            "12345",
            0,
            &vcfg_with_bindings(&[("1", "say hi")]),
        );

        let scan = scan_existing_binds(&cfg_dir);
        assert_eq!(scan.sources.len(), 1, "empty slot files must be dropped");
        assert!(scan.sources[0]
            .display_name
            .contains("cs2_user_keys_0_slot0.vcfg"));
        assert_eq!(scan.total_binds(), 1);
    }
}
