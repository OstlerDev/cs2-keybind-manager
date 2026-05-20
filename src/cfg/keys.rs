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
            "semicolon",
            "apostrophe",
            "comma",
            "period",
            "slash",
            "backslash",
            "leftbracket",
            "rightbracket",
            "minus",
            "equal",
            "backquote",
        ];
        for token in named {
            s.insert(token.into());
        }

        s
    })
}

/// Returns true if `token` is a CS2 key token we recognize.
///
/// Comparison is case-insensitive: `F1` and `f1` are both accepted, and the
/// caller is free to normalize on the way to disk.
pub fn is_valid_key_token(token: &str) -> bool {
    let lower = token.trim().to_ascii_lowercase();
    token_set().contains(&lower)
}

/// Normalizes a token to the form CS2 expects: lowercase and trimmed.
pub fn normalize(token: &str) -> String {
    token.trim().to_ascii_lowercase()
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
        assert!(is_valid_key_token("semicolon"));
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
}
