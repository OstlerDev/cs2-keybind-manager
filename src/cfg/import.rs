//! Pure parser for Valve's `cs2_user_keys_*_slot*.vcfg` keybind files.
//!
//! These files are nested KeyValues text:
//!
//! ```text
//! "config"
//! {
//!     "bindings"
//!     {
//!         "MOUSE4"  "+voicerecord"
//!         "x"       "say bark"
//!         …
//!     }
//! }
//! ```
//!
//! [`parse_keyvalues_bindings`] extracts every `"<key>" "<command>"` pair
//! inside the `bindings` section and ignores everything else (root convars,
//! sibling sections like `"analogbindings"`, etc.). Punctuation keys stored
//! as raw symbols (`[`, `]`, `,` …) are translated to their textual token
//! names by [`crate::cfg::keys::normalize`].
//!
//! This module is I/O-free: the disk walk lives in
//! [`crate::io::scan_existing_binds`].

use crate::cfg::keys;
use crate::model::BindKind;

/// One bind extracted from a source file, in a shape ready to drop into a
/// [`crate::model::KeyBind`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedBind {
    /// CS2 key token, normalized to lowercase and translated from any
    /// symbol form (`[` → `leftbracket`).
    pub key: String,
    /// Message text. For `Chat` binds, the leading `say ` has already been
    /// stripped; for `Raw` binds, this is the verbatim command text.
    pub message: String,
    pub kind: BindKind,
    /// 1-indexed line number in the original file. Surface this in the
    /// import dialog so the user can locate a bind in context.
    pub source_line: usize,
}

/// Parse a Valve KeyValues bindings file and extract every keybind from
/// the `bindings` section.
///
/// Pairs whose key isn't a known CS2 token are silently dropped. The
/// parser never panics or returns an error: anything it can't make sense
/// of, it ignores.
pub fn parse_keyvalues_bindings(content: &str) -> Vec<ParsedBind> {
    let mut out = Vec::new();
    let mut depth: i32 = 0;
    // Name of the section we just saw; consumed when the next `{` arrives.
    let mut pending_section: Option<String> = None;
    // Depth at which the `bindings` block opened; `None` means we're not
    // inside it. We track depth even when not inside so nested `{ … }`
    // blocks don't confuse the bookkeeping.
    let mut bindings_depth: Option<i32> = None;

    for (idx, raw_line) in content.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        if trimmed == "{" {
            depth += 1;
            if let Some(section) = pending_section.take() {
                if section.eq_ignore_ascii_case("bindings") && bindings_depth.is_none() {
                    bindings_depth = Some(depth);
                }
            }
            continue;
        }
        if trimmed == "}" {
            if let Some(bd) = bindings_depth {
                if depth == bd {
                    bindings_depth = None;
                }
            }
            depth -= 1;
            pending_section = None;
            continue;
        }

        if let Some((first, second)) = read_kv_pair(trimmed) {
            if bindings_depth.is_some() {
                let key = keys::normalize(&first);
                if !keys::is_valid_key_token(&key) {
                    pending_section = None;
                    continue;
                }
                let (kind, message) = match strip_say_prefix(&second) {
                    Some(body) => (BindKind::Chat, body.to_string()),
                    None => (BindKind::Raw, second),
                };
                out.push(ParsedBind {
                    key,
                    message,
                    kind,
                    source_line: line_no,
                });
            }
            pending_section = None;
        } else if let Some(name) = read_kv_section_name(trimmed) {
            pending_section = Some(name);
        }
    }

    out
}

/// Match `"key" "value"` (with arbitrary inter-token whitespace). Anything
/// after the closing quote must be blank or a `//` comment, otherwise we
/// treat the line as malformed and return `None`.
fn read_kv_pair(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix('"')?;
    let key_end = rest.find('"')?;
    let key = rest[..key_end].to_string();
    let after_key = rest[key_end + 1..].trim_start();
    let after_open = after_key.strip_prefix('"')?;
    let val_end = after_open.find('"')?;
    let value = after_open[..val_end].to_string();
    let trailing = after_open[val_end + 1..].trim();
    if !trailing.is_empty() && !trailing.starts_with("//") {
        return None;
    }
    Some((key, value))
}

/// Match a bare `"section"` line that introduces a `{ … }` block.
fn read_kv_section_name(line: &str) -> Option<String> {
    let rest = line.strip_prefix('"')?;
    let end = rest.find('"')?;
    let name = rest[..end].to_string();
    let trailing = rest[end + 1..].trim();
    if !trailing.is_empty() && !trailing.starts_with("//") {
        return None;
    }
    Some(name)
}

/// Returns the body after `say ` (case-insensitive prefix) with any
/// leading whitespace trimmed. `say_team`, `say_extra`, etc. don't match
/// because the prefix requires a literal trailing whitespace character.
fn strip_say_prefix(cmd: &str) -> Option<&str> {
    let bytes = cmd.as_bytes();
    if bytes.len() < 4 {
        return None;
    }
    if !bytes[..3].eq_ignore_ascii_case(b"say") {
        return None;
    }
    if !bytes[3].is_ascii_whitespace() {
        return None;
    }
    Some(cmd[4..].trim_start())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Realistic mini-replica of the `cs2_user_keys_*_slot*.vcfg` format.
    const SAMPLE_VCFG: &str = r#""config"
{
	"analogbindings"
	{
		"MOUSE_X"		"yaw"
	}
	"bindings"
	{
		"MOUSE4"		"+voicerecord"
		"x"		"say bark"
		"`"		"toggleconsole"
		"["		"say [ALL] hi"
		"F5"		"slot1"
	}
}
"#;

    #[test]
    fn extracts_binds_only_from_bindings_section() {
        let binds = parse_keyvalues_bindings(SAMPLE_VCFG);
        let keys: Vec<&str> = binds.iter().map(|b| b.key.as_str()).collect();
        // `MOUSE_X` is in `analogbindings`, so it must NOT appear here.
        // Symbol-form keys (`, [) stay as raw symbols — they're the
        // canonical CS2 form and what users see in the editor.
        assert_eq!(keys, vec!["mouse4", "x", "`", "[", "f5"]);
    }

    #[test]
    fn preserves_symbol_keys_in_canonical_raw_form() {
        let binds = parse_keyvalues_bindings(SAMPLE_VCFG);
        assert!(binds.iter().any(|b| b.key == "`"));
        assert!(binds.iter().any(|b| b.key == "["));
    }

    #[test]
    fn classifies_say_as_chat_and_strips_prefix() {
        let binds = parse_keyvalues_bindings(SAMPLE_VCFG);
        let x = binds.iter().find(|b| b.key == "x").unwrap();
        assert_eq!(x.kind, BindKind::Chat);
        assert_eq!(x.message, "bark");

        let lb = binds.iter().find(|b| b.key == "[").unwrap();
        assert_eq!(lb.kind, BindKind::Chat);
        assert_eq!(lb.message, "[ALL] hi");
    }

    #[test]
    fn classifies_non_say_as_raw() {
        let binds = parse_keyvalues_bindings(SAMPLE_VCFG);
        let f5 = binds.iter().find(|b| b.key == "f5").unwrap();
        assert_eq!(f5.kind, BindKind::Raw);
        assert_eq!(f5.message, "slot1");

        let mouse4 = binds.iter().find(|b| b.key == "mouse4").unwrap();
        assert_eq!(mouse4.kind, BindKind::Raw);
        assert_eq!(mouse4.message, "+voicerecord");
    }

    #[test]
    fn say_team_is_raw_not_chat() {
        // The Chat/Raw split keys off literal `say ` only; `say_team` and
        // any other underscore variant must surface as Raw so we don't
        // silently rewrite `say_team foo` as `say say_team foo` on export.
        let src = "\"config\"\n{\n\t\"bindings\"\n\t{\n\
            \t\t\"y\"\t\"say_team teamonly\"\n\
            \t}\n}\n";
        let binds = parse_keyvalues_bindings(src);
        assert_eq!(binds.len(), 1);
        assert_eq!(binds[0].kind, BindKind::Raw);
        assert_eq!(binds[0].message, "say_team teamonly");
    }

    #[test]
    fn ignores_unknown_key_tokens() {
        let src = "\"config\"\n{\n\t\"bindings\"\n\t{\n\
            \t\t\"hyperkey\" \"say hi\"\n\
            \t\t\"1\" \"say good\"\n\
            \t}\n}\n";
        let binds = parse_keyvalues_bindings(src);
        assert_eq!(binds.len(), 1);
        assert_eq!(binds[0].key, "1");
    }

    #[test]
    fn preserves_unicode_in_messages() {
        let src = "\"config\"\n{\n\t\"bindings\"\n\t{\n\
            \t\t\"n\"\t\"say [ALL] 𝕊𝕔𝕠𝕠𝕡𝕤: Daddy Chill.\"\n\
            \t}\n}\n";
        let binds = parse_keyvalues_bindings(src);
        assert_eq!(binds.len(), 1);
        assert_eq!(binds[0].message, "[ALL] 𝕊𝕔𝕠𝕠𝕡𝕤: Daddy Chill.");
    }

    #[test]
    fn handles_empty_bindings_section() {
        let src = "\"config\"\n{\n\t\"bindings\"\n\t{\n\t}\n}\n";
        assert!(parse_keyvalues_bindings(src).is_empty());
    }

    #[test]
    fn returns_empty_when_no_bindings_section() {
        let src = "\"config\"\n{\n\t\"foo\" \"bar\"\n}\n";
        assert!(parse_keyvalues_bindings(src).is_empty());
    }

    #[test]
    fn records_source_line_numbers() {
        let binds = parse_keyvalues_bindings(SAMPLE_VCFG);
        let mouse4 = binds.iter().find(|b| b.key == "mouse4").unwrap();
        assert_eq!(mouse4.source_line, 9);
    }
}
