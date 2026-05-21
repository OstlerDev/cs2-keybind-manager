//! Pure cfg generation and validation.
//!
//! Everything in this module is a pure function over `AppConfig`. No
//! filesystem, no clocks, no globals. The UI calls these functions to render
//! the preview pane; `io` calls the same functions to actually write to disk.
//! That shared code path is what guarantees the preview matches reality.

pub mod escape;
pub mod import;
pub mod keys;

use crate::model::{AppConfig, BindKind};
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

    /// The page's `direct_key` is set but isn't a recognized CS2 key token.
    InvalidDirectKey {
        page_index: usize,
        token: String,
    },
    /// Two pages have the same `direct_key`.
    DuplicateDirectKey {
        page_index: usize,
        other_page_index: usize,
        key: String,
    },
    /// A page's `direct_key` equals the global toggle key.
    DirectKeyEqualsToggle {
        page_index: usize,
        key: String,
    },
    /// A page's `direct_key` equals a bind key on some page (which would
    /// produce two `bind` lines for the same key in that page's exported
    /// cfg, breaking one of them).
    DirectKeyCollidesWithBind {
        direct_page_index: usize,
        bound_page_index: usize,
        bind_index: usize,
        key: String,
    },
}

impl ValidationError {
    /// Where in the UI this error originates. Returns `Some(page_index)` for
    /// errors tied to a specific page so callers can filter the list down to
    /// "errors for the page I'm currently editing".
    pub fn page(&self) -> Option<usize> {
        match self {
            Self::NoPages => None,
            Self::InvalidToggleKey { .. } => None,
            Self::EmptyPage { page_index }
            | Self::InvalidBindKey { page_index, .. }
            | Self::EmptyMessage { page_index, .. }
            | Self::DuplicateKeyOnPage { page_index, .. }
            | Self::ToggleKeyCollision { page_index, .. }
            | Self::InvalidDirectKey { page_index, .. }
            | Self::DirectKeyEqualsToggle { page_index, .. } => Some(*page_index),
            Self::DuplicateDirectKey { page_index, .. } => Some(*page_index),
            Self::DirectKeyCollidesWithBind {
                direct_page_index, ..
            } => Some(*direct_page_index),
        }
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPages => write!(f, "Add at least one page before exporting."),
            Self::EmptyPage { page_index } => write!(
                f,
                "Page {} has no binds. Add at least one bind or delete the page.",
                page_index + 1
            ),
            Self::InvalidToggleKey { token } => write!(
                f,
                "Cycle-pages key '{token}' isn't a recognized CS2 key token. \
                 Examples: F1, F2, kp_5, mouse4."
            ),
            Self::InvalidBindKey {
                page_index,
                bind_index,
                token,
            } => write!(
                f,
                "Page {}, bind {}: key '{token}' isn't a recognized CS2 key token. \
                 Examples: 1, F2, kp_5, mwheelup, semicolon.",
                page_index + 1,
                bind_index + 1
            ),
            Self::EmptyMessage {
                page_index,
                bind_index,
            } => write!(
                f,
                "Page {}, bind {}: message is empty.",
                page_index + 1,
                bind_index + 1
            ),
            Self::DuplicateKeyOnPage { page_index, key } => write!(
                f,
                "Page {} has two binds for key '{key}'. Each key can only appear once per page.",
                page_index + 1
            ),
            Self::ToggleKeyCollision {
                page_index,
                bind_index,
                key,
            } => write!(
                f,
                "Page {}, bind {}: key '{key}' is the same as the cycle-pages key. \
                 Pick a different key for this bind.",
                page_index + 1,
                bind_index + 1
            ),
            Self::InvalidDirectKey { page_index, token } => write!(
                f,
                "Page {}: direct key '{token}' isn't a recognized CS2 key token.",
                page_index + 1
            ),
            Self::DuplicateDirectKey {
                page_index,
                other_page_index,
                key,
            } => write!(
                f,
                "Pages {} and {} both use direct key '{key}'. Direct keys must be unique.",
                other_page_index + 1,
                page_index + 1
            ),
            Self::DirectKeyEqualsToggle { page_index, key } => write!(
                f,
                "Page {}: direct key '{key}' is the same as the cycle-pages key.",
                page_index + 1
            ),
            Self::DirectKeyCollidesWithBind {
                direct_page_index,
                bound_page_index,
                bind_index,
                key,
            } => write!(
                f,
                "Page {}'s direct key '{key}' collides with the bind on page {}, row {}. \
                 Pick a different key.",
                direct_page_index + 1,
                bound_page_index + 1,
                bind_index + 1
            ),
        }
    }
}

/// Returns the trimmed, normalized direct key for a page if it has one set.
fn normalized_direct_key(page: &crate::model::Page) -> Option<String> {
    page.direct_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(keys::normalize)
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

    // direct_key validation. Done in a second pass because each rule needs
    // to look across all pages.
    let mut direct_keys: std::collections::HashMap<String, usize> = Default::default();
    for (pi, page) in cfg.pages.iter().enumerate() {
        let Some(dk) = normalized_direct_key(page) else {
            continue;
        };

        if !keys::is_valid_key_token(&dk) {
            errors.push(ValidationError::InvalidDirectKey {
                page_index: pi,
                token: page.direct_key.clone().unwrap_or_default(),
            });
            continue;
        }

        if dk == normalized_toggle {
            errors.push(ValidationError::DirectKeyEqualsToggle {
                page_index: pi,
                key: dk.clone(),
            });
        }

        if let Some(&prev) = direct_keys.get(&dk) {
            errors.push(ValidationError::DuplicateDirectKey {
                page_index: pi,
                other_page_index: prev,
                key: dk.clone(),
            });
        } else {
            direct_keys.insert(dk.clone(), pi);
        }

        for (bp, bound_page) in cfg.pages.iter().enumerate() {
            for (bi, bind) in bound_page.binds.iter().enumerate() {
                if keys::normalize(&bind.key) == dk {
                    errors.push(ValidationError::DirectKeyCollidesWithBind {
                        direct_page_index: pi,
                        bound_page_index: bp,
                        bind_index: bi,
                        key: dk.clone(),
                    });
                }
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
/// Each generated file:
///   1. Binds every per-page key to its `say` message.
///   2. Binds the global cycle-pages key to `exec`-ing the next page.
///   3. Binds every page's direct-key (across the whole app) to `exec`-ing
///      that page, so the user can jump anywhere from anywhere.
///
/// This is a pure function over `cfg`. It does not validate — call
/// [`validate`] first if you want to refuse bad input.
pub fn generate_page_files(cfg: &AppConfig) -> Vec<GeneratedFile> {
    let n = cfg.pages.len();
    if n == 0 {
        return Vec::new();
    }

    let toggle = keys::normalize(&cfg.toggle_key);

    let direct_jumps: Vec<(String, usize)> = cfg
        .pages
        .iter()
        .enumerate()
        .filter_map(|(i, p)| normalized_direct_key(p).map(|k| (k, i)))
        .collect();

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
            let payload = match bind.kind {
                BindKind::Chat => format!("say {}", sanitized.message),
                BindKind::Raw => sanitized.message.clone(),
            };
            body.push_str(&format!("bind \"{}\" \"{}\"\n", key, payload));
        }

        body.push('\n');
        body.push_str(&format!(
            "bind \"{}\" \"exec {}\"\n",
            toggle,
            page_exec_name(next)
        ));

        for (dk, target_idx) in &direct_jumps {
            body.push_str(&format!(
                "bind \"{}\" \"exec {}\"\n",
                dk,
                page_exec_name(*target_idx)
            ));
        }

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
