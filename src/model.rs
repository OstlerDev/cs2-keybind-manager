//! Persistent application state.
//!
//! The model is intentionally plain data: no I/O, no `egui` types, no clocks.
//! Anything in here can be serialized to disk and round-tripped through
//! `serde_json`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Top-level state that gets persisted to `state.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Path to the user's CS2 cfg directory (`…\game\csgo\cfg`).
    pub cs2_cfg_dir: Option<PathBuf>,

    /// The key the user presses to cycle to the next page.
    pub toggle_key: String,

    /// Ordered list of pages. The cycle order in-game matches this order.
    pub pages: Vec<Page>,

    /// Index of the page currently being edited in the UI. Not exported.
    #[serde(default)]
    pub selected_page: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            cs2_cfg_dir: None,
            toggle_key: "F1".into(),
            pages: vec![Page::new("Page 1")],
            selected_page: 0,
        }
    }
}

/// A single layer of binds. All pages share the same cycle-pages key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub name: String,

    /// Optional dedicated key that jumps directly to this page from any
    /// other page. Independent of the global cycle-pages key. Empty/None
    /// means "no direct shortcut for this page".
    #[serde(default)]
    pub direct_key: Option<String>,

    pub binds: Vec<KeyBind>,
}

impl Page {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            direct_key: None,
            binds: Vec::new(),
        }
    }
}

/// One row in the bind editor: a CS2 key token mapped to a chat message.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeyBind {
    pub key: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_one_empty_starter_page() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.toggle_key, "F1");
        assert_eq!(cfg.pages.len(), 1);
        assert_eq!(cfg.pages[0].name, "Page 1");
        assert!(cfg.pages[0].binds.is_empty());
        assert_eq!(cfg.selected_page, 0);
    }

    #[test]
    fn serde_round_trip() {
        let cfg = AppConfig {
            cs2_cfg_dir: Some(PathBuf::from("C:/cs2/cfg")),
            toggle_key: "F2".into(),
            pages: vec![Page {
                name: "Strat Calls".into(),
                direct_key: Some("F3".into()),
                binds: vec![KeyBind {
                    key: "1".into(),
                    message: "Rush B".into(),
                }],
            }],
            selected_page: 0,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.toggle_key, "F2");
        assert_eq!(back.pages[0].binds[0].message, "Rush B");
        assert_eq!(back.pages[0].direct_key.as_deref(), Some("F3"));
    }

    #[test]
    fn legacy_state_files_with_extra_fields_load_cleanly() {
        // Old state.json (from a build that had display_name) should still
        // load: serde silently drops unknown fields by default.
        let json = r#"{
            "cs2_cfg_dir": null,
            "toggle_key": "F1",
            "pages": [{
                "name": "Old Page",
                "display_name": "STRAT",
                "binds": [{"key": "1", "message": "hi"}]
            }]
        }"#;
        let cfg: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.pages[0].name, "Old Page");
        assert_eq!(cfg.pages[0].direct_key, None);
    }

    #[test]
    fn missing_selected_page_field_defaults_to_zero() {
        let json = r#"{
            "cs2_cfg_dir": null,
            "toggle_key": "F1",
            "pages": []
        }"#;
        let cfg: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.selected_page, 0);
    }

    #[test]
    fn old_state_files_without_direct_key_load_cleanly() {
        let json = r#"{
            "cs2_cfg_dir": null,
            "toggle_key": "F1",
            "pages": [{
                "name": "Old Page",
                "binds": [{"key": "1", "message": "hi"}]
            }]
        }"#;
        let cfg: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.pages[0].direct_key, None);
    }
}
