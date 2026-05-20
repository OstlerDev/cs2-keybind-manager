//! Pure cfg generation and validation.
//!
//! Everything in this module is a pure function over `AppConfig`. No
//! filesystem, no clocks, no globals. The UI calls these functions to render
//! the preview pane; `io` calls the same functions to actually write to disk.
//! That shared code path is what guarantees the preview matches reality.

pub mod escape;
pub mod keys;

use crate::model::AppConfig;
use escape::{sanitize_chat_message, SanitizationWarning};

/// Marker that fences off the block we own inside `autoexec.cfg`. Anything
/// outside these markers is left strictly untouched.
pub const AUTOEXEC_BEGIN: &str = "// === CS2_KEYBIND_MANAGER_START ===";
pub const AUTOEXEC_END: &str = "// === CS2_KEYBIND_MANAGER_END ===";

/// Filename prefix for our generated per-page cfgs. Namespacing keeps us out
/// of the user's existing config files and makes manual cleanup trivial.
pub const PAGE_FILE_PREFIX: &str = "bindmgr_page_";

/// The first page is loaded by `autoexec.cfg` on game launch.
pub fn page_filename(page_index_zero_based: usize) -> String {
    format!("{PAGE_FILE_PREFIX}{}.cfg", page_index_zero_based + 1)
}

/// The bare cfg name (no extension) used inside `exec` directives.
pub fn page_exec_name(page_index_zero_based: usize) -> String {
    format!("{PAGE_FILE_PREFIX}{}", page_index_zero_based + 1)
}

/// One generated file. `filename` is just the basename; the caller decides
/// where to put it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedFile {
    pub filename: String,
    pub contents: String,
    pub warnings: Vec<MessageWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageWarning {
    pub page_index: usize,
    pub bind_index: usize,
    pub kind: SanitizationWarning,
}

/// Validation errors block Export. Each variant points at the offending bind
/// or page so the UI can highlight it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    NoPages,
    EmptyPage {
        page_index: usize,
    },
    InvalidToggleKey {
        token: String,
    },
    InvalidBindKey {
        page_index: usize,
        bind_index: usize,
        token: String,
    },
    EmptyMessage {
        page_index: usize,
        bind_index: usize,
    },
    DuplicateKeyOnPage {
        page_index: usize,
        key: String,
    },
    ToggleKeyCollision {
        page_index: usize,
        bind_index: usize,
        key: String,
    },
}

/// Run every validation rule. Returns *all* errors so the UI can show them
/// inline rather than making the user fix one at a time.
pub fn validate(cfg: &AppConfig) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    if cfg.pages.is_empty() {
        errors.push(ValidationError::NoPages);
    }

    let normalized_toggle = keys::normalize(&cfg.toggle_key);
    if !keys::is_valid_key_token(&normalized_toggle) {
        errors.push(ValidationError::InvalidToggleKey {
            token: cfg.toggle_key.clone(),
        });
    }

    for (pi, page) in cfg.pages.iter().enumerate() {
        if page.binds.is_empty() {
            errors.push(ValidationError::EmptyPage { page_index: pi });
        }

        let mut seen: std::collections::HashMap<String, usize> = Default::default();

        for (bi, bind) in page.binds.iter().enumerate() {
            let normalized_key = keys::normalize(&bind.key);

            if !keys::is_valid_key_token(&normalized_key) {
                errors.push(ValidationError::InvalidBindKey {
                    page_index: pi,
                    bind_index: bi,
                    token: bind.key.clone(),
                });
                continue;
            }

            if bind.message.trim().is_empty() {
                errors.push(ValidationError::EmptyMessage {
                    page_index: pi,
                    bind_index: bi,
                });
            }

            if normalized_key == normalized_toggle {
                errors.push(ValidationError::ToggleKeyCollision {
                    page_index: pi,
                    bind_index: bi,
                    key: normalized_key.clone(),
                });
            }

            if let Some(_first) = seen.insert(normalized_key.clone(), bi) {
                errors.push(ValidationError::DuplicateKeyOnPage {
                    page_index: pi,
                    key: normalized_key,
                });
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Generates one `.cfg` file per page. The list is in cycle order: index 0 is
/// page 1, which is also what `autoexec.cfg` execs at launch.
///
/// This is a pure function over `cfg`. It does not validate — call
/// [`validate`] first if you want to refuse bad input.
pub fn generate_page_files(cfg: &AppConfig) -> Vec<GeneratedFile> {
    let n = cfg.pages.len();
    if n == 0 {
        return Vec::new();
    }

    let toggle = keys::normalize(&cfg.toggle_key);
    let mut out = Vec::with_capacity(n);

    for (i, page) in cfg.pages.iter().enumerate() {
        let next = (i + 1) % n;
        let mut body = String::new();
        let mut warnings = Vec::new();

        body.push_str("// Generated by CS2 Keybind Manager. Do not edit by hand.\n");
        body.push_str(&format!("// Page {} of {}: {}\n\n", i + 1, n, page.name));

        for (bi, bind) in page.binds.iter().enumerate() {
            let key = keys::normalize(&bind.key);
            let sanitized = sanitize_chat_message(&bind.message);
            for w in sanitized.warnings {
                warnings.push(MessageWarning {
                    page_index: i,
                    bind_index: bi,
                    kind: w,
                });
            }
            body.push_str(&format!("bind \"{}\" \"say {}\"\n", key, sanitized.message));
        }

        body.push('\n');
        body.push_str(&format!(
            "bind \"{}\" \"exec {}\"\n",
            toggle,
            page_exec_name(next)
        ));

        out.push(GeneratedFile {
            filename: page_filename(i),
            contents: body,
            warnings,
        });
    }

    out
}

/// Idempotently inserts or refreshes our marker block inside `autoexec.cfg`.
///
/// Anything outside the markers is preserved verbatim. If no marker block is
/// present, ours is appended (with a leading blank line for readability).
/// Running the function twice on the same input produces no further change.
pub fn update_autoexec(existing: &str, cfg: &AppConfig) -> String {
    let block = build_autoexec_block(cfg);

    match (existing.find(AUTOEXEC_BEGIN), existing.find(AUTOEXEC_END)) {
        (Some(start), Some(end_marker_start)) if end_marker_start > start => {
            let end = end_marker_start + AUTOEXEC_END.len();
            let mut out = String::with_capacity(existing.len());
            out.push_str(&existing[..start]);
            out.push_str(&block);
            out.push_str(&existing[end..]);
            out
        }
        _ => {
            let mut out = existing.to_owned();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&block);
            out.push('\n');
            out
        }
    }
}

fn build_autoexec_block(cfg: &AppConfig) -> String {
    let mut s = String::new();
    s.push_str(AUTOEXEC_BEGIN);
    s.push('\n');
    s.push_str("// Managed by CS2 Keybind Manager. Edits inside this block will be overwritten.\n");
    if cfg.pages.is_empty() {
        s.push_str("// (No pages configured.)\n");
    } else {
        s.push_str(&format!("exec {}\n", page_exec_name(0)));
    }
    s.push_str(AUTOEXEC_END);
    s
}

#[cfg(test)]
mod tests;
