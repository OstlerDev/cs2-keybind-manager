//! Recognized CS2 key tokens.
//!
//! CS2 inherits the Source-engine key-binding token vocabulary. This module
//! holds the curated allow-list and the validator we run before exporting.
//! The list is intentionally exhaustive within categories (function keys,
//! keypad, mouse, modifiers, common control keys) but does not pretend to
//! cover every exotic peripheral input.

use std::collections::HashSet;
use std::sync::OnceLock;

fn token_set() -> &'static HashSet<String> {
    static SET: OnceLock<HashSet<String>> = OnceLock::new();
    SET.get_or_init(|| {
        let mut s = HashSet::new();

        for c in 'a'..='z' {
            s.insert(c.to_string());
        }
        for c in '0'..='9' {
            s.insert(c.to_string());
        }
        for n in 1..=12u8 {
            s.insert(format!("f{n}"));
        }

        let named = [
            "kp_ins",
            "kp_end",
            "kp_downarrow",
            "kp_pgdn",
            "kp_leftarrow",
            "kp_5",
            "kp_rightarrow",
            "kp_home",
            "kp_uparrow",
            "kp_pgup",
            "kp_slash",
            "kp_multiply",
            "kp_minus",
            "kp_plus",
            "kp_enter",
            "kp_del",
            "kp_0",
            "kp_1",
            "kp_2",
            "kp_3",
            "kp_4",
            "kp_6",
            "kp_7",
            "kp_8",
            "kp_9",
            "mouse1",
            "mouse2",
            "mouse3",
            "mouse4",
            "mouse5",
            "mwheelup",
            "mwheeldown",
            "shift",
            "ctrl",
            "alt",
            "space",
            "enter",
            "escape",
            "tab",
            "backspace",
            "capslock",
            "pause",
            "scrolllock",
            "uparrow",
            "downarrow",
            "leftarrow",
            "rightarrow",
            "ins",
            "del",
            "home",
            "end",
            "pgup",
            "pgdn",
            // Punctuation keys are stored in their raw-symbol form, matching
            // what CS2 itself writes into cs2_user_keys_*_slot*.vcfg. Their
            // textual aliases (`leftbracket`, `comma`, …) still validate
            // through the alias table in `normalize`.
            "[",
            "]",
            ",",
            ".",
            "/",
            "\\",
            "'",
            ";",
            "`",
            "-",
            "=",
        ];
        for token in named {
            s.insert(token.into());
        }

        s
    })
}

/// Returns true if `token` is a CS2 key token we recognize.
///
/// Comparison runs through [`normalize`], so both case variants (`F1` /
/// `f1`) and both alias forms (`[` / `leftbracket`) are accepted.
pub fn is_valid_key_token(token: &str) -> bool {
    token_set().contains(&normalize(token))
}

/// Normalizes a token to its canonical CS2 form.
///
/// Trims surrounding whitespace, lowercases ASCII letters, and resolves
/// the legacy Source-engine textual aliases for punctuation keys
/// (e.g. `leftbracket` → `[`) to the raw-symbol form CS2 now uses
/// natively. The raw-symbol form is what you see in the import dialog,
/// the bind editor, and the emitted `bind "<key>" "<cmd>"` lines.
pub fn normalize(token: &str) -> String {
    let lower = token.trim().to_ascii_lowercase();
    match lower.as_str() {
        "leftbracket" => "[".into(),
        "rightbracket" => "]".into(),
        "comma" => ",".into(),
        "period" => ".".into(),
        "slash" => "/".into(),
        "backslash" => "\\".into(),
        "apostrophe" => "'".into(),
        "semicolon" => ";".into(),
        "backquote" => "`".into(),
        "minus" => "-".into(),
        "equal" => "=".into(),
        _ => lower,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_letters_and_digits() {
        assert!(is_valid_key_token("a"));
        assert!(is_valid_key_token("Z"));
        assert!(is_valid_key_token("0"));
        assert!(is_valid_key_token("9"));
    }

    #[test]
    fn accepts_function_keys_case_insensitive() {
        assert!(is_valid_key_token("F1"));
        assert!(is_valid_key_token("f12"));
        assert!(!is_valid_key_token("f13"));
        assert!(!is_valid_key_token("f0"));
    }

    #[test]
    fn accepts_keypad_tokens() {
        assert!(is_valid_key_token("kp_5"));
        assert!(is_valid_key_token("kp_multiply"));
        assert!(is_valid_key_token("kp_enter"));
    }

    #[test]
    fn accepts_mouse_and_wheel() {
        assert!(is_valid_key_token("mouse4"));
        assert!(is_valid_key_token("mwheelup"));
        assert!(is_valid_key_token("mwheeldown"));
        assert!(!is_valid_key_token("mouse99"));
    }

    #[test]
    fn accepts_named_control_keys() {
        assert!(is_valid_key_token("space"));
        assert!(is_valid_key_token("shift"));
        assert!(is_valid_key_token("uparrow"));
    }

    #[test]
    fn rejects_unknown_tokens() {
        assert!(!is_valid_key_token("hyperkey"));
        assert!(!is_valid_key_token(""));
        assert!(!is_valid_key_token("a b"));
    }

    #[test]
    fn normalize_lowercases_and_trims() {
        assert_eq!(normalize("  F1 "), "f1");
        assert_eq!(normalize("KP_Multiply"), "kp_multiply");
    }

    #[test]
    fn normalize_resolves_textual_aliases_to_raw_symbols() {
        // Legacy Source-engine textual names collapse to the raw symbol
        // form CS2 now uses natively in vcfg files and the bind editor.
        assert_eq!(normalize("leftbracket"), "[");
        assert_eq!(normalize("rightbracket"), "]");
        assert_eq!(normalize("comma"), ",");
        assert_eq!(normalize("period"), ".");
        assert_eq!(normalize("slash"), "/");
        assert_eq!(normalize("backslash"), "\\");
        assert_eq!(normalize("apostrophe"), "'");
        assert_eq!(normalize("semicolon"), ";");
        assert_eq!(normalize("backquote"), "`");
        assert_eq!(normalize("minus"), "-");
        assert_eq!(normalize("equal"), "=");
    }

    #[test]
    fn aliases_are_case_insensitive() {
        assert_eq!(normalize("LeftBracket"), "[");
        assert_eq!(normalize("SEMICOLON"), ";");
    }

    #[test]
    fn normalize_passes_symbol_form_through_unchanged() {
        // The symbol form is already canonical; normalize must be a
        // no-op on it. (Idempotency keeps repeated saves stable.)
        for sym in ["[", "]", ",", ".", "/", "\\", "'", ";", "`", "-", "="] {
            assert_eq!(normalize(sym), sym);
        }
    }

    #[test]
    fn both_forms_validate_as_known_tokens() {
        // Symbol form (canonical):
        assert!(is_valid_key_token("["));
        assert!(is_valid_key_token("]"));
        assert!(is_valid_key_token("`"));
        assert!(is_valid_key_token("="));
        // Textual aliases still validate so legacy state.json files and
        // hand-typed inputs keep working.
        assert!(is_valid_key_token("leftbracket"));
        assert!(is_valid_key_token("semicolon"));
        assert!(is_valid_key_token("BACKQUOTE"));
    }
}
