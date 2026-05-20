//! Chat-message sanitization for the `bind <key> "say <msg>"` form.
//!
//! CS2's binder cannot handle nested double quotes — a message containing `"`
//! breaks the parse. We strip them at export time and emit a warning so the
//! preview pane can show the user exactly which messages were modified.
//!
//! Newlines and carriage returns are also stripped: chat messages in CS2 are
//! single-line, and a stray newline would terminate the `bind` line early.

/// Result of sanitizing a single message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sanitized {
    pub message: String,
    pub warnings: Vec<SanitizationWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SanitizationWarning {
    StrippedDoubleQuotes,
    StrippedNewlines,
}

/// Returns the safe form of `raw` plus any warnings about what was changed.
pub fn sanitize_chat_message(raw: &str) -> Sanitized {
    let mut warnings = Vec::new();
    let mut out = String::with_capacity(raw.len());

    let mut had_quote = false;
    let mut had_newline = false;

    for ch in raw.chars() {
        match ch {
            '"' => had_quote = true,
            '\n' | '\r' => had_newline = true,
            _ => out.push(ch),
        }
    }

    if had_quote {
        warnings.push(SanitizationWarning::StrippedDoubleQuotes);
    }
    if had_newline {
        warnings.push(SanitizationWarning::StrippedNewlines);
    }

    Sanitized {
        message: out,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_on_safe_message() {
        let r = sanitize_chat_message("Rush B, don't stop!");
        assert_eq!(r.message, "Rush B, don't stop!");
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn strips_double_quotes_with_warning() {
        let r = sanitize_chat_message(r#"He said "go A" to me"#);
        assert_eq!(r.message, "He said go A to me");
        assert_eq!(r.warnings, vec![SanitizationWarning::StrippedDoubleQuotes]);
    }

    #[test]
    fn strips_newlines_with_warning() {
        let r = sanitize_chat_message("line one\nline two\rline three");
        assert_eq!(r.message, "line oneline twoline three");
        assert_eq!(r.warnings, vec![SanitizationWarning::StrippedNewlines]);
    }

    #[test]
    fn reports_both_warnings_when_both_present() {
        let r = sanitize_chat_message("\"yo\"\n\"hi\"");
        assert_eq!(r.message, "yohi");
        assert_eq!(
            r.warnings,
            vec![
                SanitizationWarning::StrippedDoubleQuotes,
                SanitizationWarning::StrippedNewlines
            ]
        );
    }

    #[test]
    fn empty_input_produces_empty_output() {
        let r = sanitize_chat_message("");
        assert_eq!(r.message, "");
        assert!(r.warnings.is_empty());
    }
}
