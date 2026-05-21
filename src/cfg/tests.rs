//! Tests for the pure cfg module.
//!
//! Snapshot tests cover the generator (the most likely thing to drift); plain
//! unit tests cover validation and the autoexec updater (where exact strings
//! and idempotency matter more than overall shape).

use super::*;
use crate::model::{AppConfig, BindKind, KeyBind, Page};
use std::path::PathBuf;

fn page(name: &str, binds: &[(&str, &str)]) -> Page {
    Page {
        name: name.into(),
        direct_key: None,
        binds: binds
            .iter()
            .map(|(k, m)| KeyBind {
                key: (*k).into(),
                message: (*m).into(),
                ..KeyBind::default()
            })
            .collect(),
    }
}

fn page_with(name: &str, direct: Option<&str>, binds: &[(&str, &str)]) -> Page {
    let mut p = page(name, binds);
    p.direct_key = direct.map(Into::into);
    p
}

fn cfg_with(pages: Vec<Page>, toggle: &str) -> AppConfig {
    AppConfig {
        cs2_cfg_dir: Some(PathBuf::from("C:/cs2/cfg")),
        toggle_key: toggle.into(),
        pages,
        ..AppConfig::default()
    }
}

mod page_filenames {
    use super::*;

    #[test]
    fn filenames_are_one_indexed() {
        assert_eq!(page_filename(0), "bindmgr_page_1.cfg");
        assert_eq!(page_filename(2), "bindmgr_page_3.cfg");
    }

    #[test]
    fn exec_names_strip_extension() {
        assert_eq!(page_exec_name(0), "bindmgr_page_1");
        assert_eq!(page_exec_name(4), "bindmgr_page_5");
    }
}

mod generator {
    use super::*;

    fn render(files: &[GeneratedFile]) -> String {
        let mut s = String::new();
        for f in files {
            s.push_str(&format!("=== {} ===\n", f.filename));
            s.push_str(&f.contents);
            if !f.warnings.is_empty() {
                s.push_str("--- warnings ---\n");
                for w in &f.warnings {
                    s.push_str(&format!(
                        "page={} bind={} {:?}\n",
                        w.page_index, w.bind_index, w.kind
                    ));
                }
            }
            s.push('\n');
        }
        s
    }

    #[test]
    fn empty_app_produces_no_files() {
        let cfg = cfg_with(vec![], "F1");
        assert!(generate_page_files(&cfg).is_empty());
    }

    #[test]
    fn single_page_cycles_back_to_itself() {
        let cfg = cfg_with(vec![page("Solo", &[("1", "Hi")])], "F1");
        let files = generate_page_files(&cfg);
        insta::assert_snapshot!("generator__single_page", render(&files));
    }

    #[test]
    fn three_page_cycle() {
        let cfg = cfg_with(
            vec![
                page_with(
                    "Strat Calls",
                    Some("F2"),
                    &[("1", "Rush B"), ("2", "Smoke mid")],
                ),
                page_with("Trash Talk", Some("F3"), &[("1", "ez"), ("2", "ggwp")]),
                page("Compliments", &[("1", "Nice shot!"), ("2", "Good half")]),
            ],
            "F1",
        );
        let files = generate_page_files(&cfg);
        assert_eq!(files.len(), 3);
        insta::assert_snapshot!("generator__three_page_cycle", render(&files));
    }

    #[test]
    fn keys_and_toggle_are_normalized_to_lowercase() {
        let cfg = cfg_with(vec![page("P", &[("KP_5", "msg"), ("F2", "other")])], "f1");
        let files = generate_page_files(&cfg);
        let body = &files[0].contents;
        assert!(body.contains("bind \"kp_5\" \"say msg\""));
        assert!(body.contains("bind \"f2\" \"say other\""));
        assert!(body.contains("bind \"f1\" \"exec bindmgr_page_1\""));
    }

    #[test]
    fn quotes_in_message_are_stripped_with_warning() {
        let cfg = cfg_with(vec![page("P", &[("1", r#"He said "rush" now"#)])], "F1");
        let files = generate_page_files(&cfg);
        let body = &files[0].contents;
        assert!(body.contains("bind \"1\" \"say He said rush now\""));
        assert_eq!(files[0].warnings.len(), 1);
        assert_eq!(
            files[0].warnings[0].kind,
            escape::SanitizationWarning::StrippedDoubleQuotes
        );
    }

    #[test]
    fn no_echo_lines_are_emitted() {
        // We deliberately don't emit `echo` lines: CS2 only renders them in
        // the console (not the HUD) without screenmessage_show, which is
        // cheat-protected on Valve servers.
        let cfg = cfg_with(vec![page("P", &[("1", "Rush B")])], "F1");
        let body = &generate_page_files(&cfg)[0].contents;
        assert!(!body.contains("echo "), "no echo lines, got: {body}");
    }

    #[test]
    fn mixed_chat_and_raw_binds_render_correctly() {
        let cfg = cfg_with(
            vec![Page {
                binds: vec![
                    KeyBind {
                        key: "1".into(),
                        message: "Rush B".into(),
                        kind: BindKind::Chat,
                    },
                    KeyBind {
                        key: "2".into(),
                        message: "slot1".into(),
                        kind: BindKind::Raw,
                    },
                    KeyBind {
                        key: "3".into(),
                        message: "say_team Hold A".into(),
                        kind: BindKind::Raw,
                    },
                ],
                ..Page::new("Mixed")
            }],
            "F1",
        );
        let files = generate_page_files(&cfg);
        let body = &files[0].contents;
        assert!(body.contains(r#"bind "1" "say Rush B""#));
        assert!(body.contains(r#"bind "2" "slot1""#));
        assert!(body.contains(r#"bind "3" "say_team Hold A""#));
        insta::assert_snapshot!("generator__mixed_kinds", render(&files));
    }

    #[test]
    fn direct_keys_appear_in_every_page() {
        let cfg = cfg_with(
            vec![
                page_with("A", Some("F2"), &[("1", "a")]),
                page_with("B", Some("F3"), &[("1", "b")]),
                page("C", &[("1", "c")]),
            ],
            "F1",
        );
        let files = generate_page_files(&cfg);
        for f in &files {
            assert!(f.contents.contains(r#"bind "f2" "exec bindmgr_page_1""#));
            assert!(f.contents.contains(r#"bind "f3" "exec bindmgr_page_2""#));
        }
    }
}

mod validation {
    use super::*;

    fn err(cfg: &AppConfig) -> Vec<ValidationError> {
        validate(cfg).unwrap_err()
    }

    #[test]
    fn happy_path_passes() {
        let cfg = cfg_with(
            vec![page("A", &[("1", "hi")]), page("B", &[("1", "yo")])],
            "F1",
        );
        assert!(validate(&cfg).is_ok());
    }

    #[test]
    fn no_pages_is_an_error() {
        let cfg = cfg_with(vec![], "F1");
        assert!(err(&cfg).contains(&ValidationError::NoPages));
    }

    #[test]
    fn empty_page_is_an_error() {
        let cfg = cfg_with(vec![page("Empty", &[])], "F1");
        assert!(err(&cfg).contains(&ValidationError::EmptyPage { page_index: 0 }));
    }

    #[test]
    fn invalid_toggle_key_is_an_error() {
        let cfg = cfg_with(vec![page("P", &[("1", "hi")])], "hyperspace");
        assert!(err(&cfg)
            .iter()
            .any(|e| matches!(e, ValidationError::InvalidToggleKey { .. })));
    }

    #[test]
    fn invalid_bind_key_is_an_error() {
        let cfg = cfg_with(vec![page("P", &[("nope_key", "hi")])], "F1");
        assert!(err(&cfg)
            .iter()
            .any(|e| matches!(e, ValidationError::InvalidBindKey { .. })));
    }

    #[test]
    fn empty_message_is_an_error() {
        let cfg = cfg_with(vec![page("P", &[("1", "   ")])], "F1");
        assert!(err(&cfg)
            .iter()
            .any(|e| matches!(e, ValidationError::EmptyMessage { .. })));
    }

    #[test]
    fn duplicate_key_within_page_is_an_error() {
        let cfg = cfg_with(vec![page("P", &[("1", "a"), ("1", "b")])], "F1");
        let errs = err(&cfg);
        assert!(errs
            .iter()
            .any(|e| matches!(e, ValidationError::DuplicateKeyOnPage { page_index: 0, .. })));
    }

    #[test]
    fn toggle_collision_with_page_bind_is_an_error() {
        let cfg = cfg_with(vec![page("P", &[("F1", "hi")])], "F1");
        assert!(err(&cfg)
            .iter()
            .any(|e| matches!(e, ValidationError::ToggleKeyCollision { .. })));
    }

    #[test]
    fn errors_are_aggregated_not_fail_fast() {
        let cfg = cfg_with(
            vec![page("Bad", &[("nope", ""), ("1", "ok")])],
            "hyperspace",
        );
        let errs = err(&cfg);
        assert!(errs
            .iter()
            .any(|e| matches!(e, ValidationError::InvalidToggleKey { .. })));
        assert!(errs
            .iter()
            .any(|e| matches!(e, ValidationError::InvalidBindKey { .. })));
    }

    #[test]
    fn direct_key_can_be_unset() {
        let cfg = cfg_with(vec![page("P", &[("1", "hi")])], "F1");
        assert!(validate(&cfg).is_ok());
    }

    #[test]
    fn invalid_direct_key_is_an_error() {
        let cfg = cfg_with(
            vec![page_with("P", Some("hyperspace"), &[("1", "hi")])],
            "F1",
        );
        assert!(err(&cfg)
            .iter()
            .any(|e| matches!(e, ValidationError::InvalidDirectKey { page_index: 0, .. })));
    }

    #[test]
    fn duplicate_direct_keys_are_an_error() {
        let cfg = cfg_with(
            vec![
                page_with("A", Some("F2"), &[("1", "a")]),
                page_with("B", Some("F2"), &[("1", "b")]),
            ],
            "F1",
        );
        assert!(err(&cfg)
            .iter()
            .any(|e| matches!(e, ValidationError::DuplicateDirectKey { .. })));
    }

    #[test]
    fn direct_key_equal_to_toggle_is_an_error() {
        let cfg = cfg_with(vec![page_with("A", Some("F1"), &[("1", "a")])], "F1");
        assert!(err(&cfg)
            .iter()
            .any(|e| matches!(e, ValidationError::DirectKeyEqualsToggle { .. })));
    }

    #[test]
    fn direct_key_colliding_with_a_bind_key_is_an_error() {
        let cfg = cfg_with(
            vec![
                page_with("A", Some("kp_5"), &[("1", "a")]),
                page("B", &[("kp_5", "b")]),
            ],
            "F1",
        );
        assert!(err(&cfg)
            .iter()
            .any(|e| matches!(e, ValidationError::DirectKeyCollidesWithBind { .. })));
    }

    #[test]
    fn whitespace_only_direct_key_is_treated_as_unset() {
        let cfg = cfg_with(vec![page_with("A", Some("   "), &[("1", "a")])], "F1");
        assert!(validate(&cfg).is_ok());
    }
}

mod autoexec {
    use super::*;

    fn cfg() -> AppConfig {
        cfg_with(vec![page("P", &[("1", "hi")])], "F1")
    }

    #[test]
    fn appends_block_to_empty_file() {
        let out = update_autoexec("", &cfg());
        assert!(out.contains(AUTOEXEC_BEGIN));
        assert!(out.contains(AUTOEXEC_END));
        assert!(out.contains("exec bindmgr_page_1"));
    }

    #[test]
    fn appends_block_to_file_without_marker() {
        let existing = "// my settings\nsensitivity 1.5\n";
        let out = update_autoexec(existing, &cfg());
        assert!(out.starts_with("// my settings\nsensitivity 1.5\n"));
        assert!(out.contains(AUTOEXEC_BEGIN));
        assert!(out.contains("exec bindmgr_page_1"));
    }

    #[test]
    fn idempotent_when_block_already_correct() {
        let cfg = cfg();
        let once = update_autoexec("// pre\n", &cfg);
        let twice = update_autoexec(&once, &cfg);
        assert_eq!(once, twice);
    }

    #[test]
    fn refreshes_block_in_place_preserving_surroundings() {
        let cfg = cfg();
        let pre = "// my pre setting\nsensitivity 1.5\n";
        let post = "\n// my post setting\nfps_max 0\n";

        let stale_block =
            format!("{AUTOEXEC_BEGIN}\nexec something_old\nexec another_old\n{AUTOEXEC_END}");
        let existing = format!("{pre}{stale_block}{post}");

        let out = update_autoexec(&existing, &cfg);

        assert!(
            out.starts_with(pre),
            "pre-block content must survive verbatim"
        );
        assert!(
            out.ends_with(post),
            "post-block content must survive verbatim"
        );
        assert!(!out.contains("exec something_old"));
        assert!(!out.contains("exec another_old"));
        assert!(out.contains("exec bindmgr_page_1"));
        let begins = out.matches(AUTOEXEC_BEGIN).count();
        let ends = out.matches(AUTOEXEC_END).count();
        assert_eq!(begins, 1);
        assert_eq!(ends, 1);
    }

    #[test]
    fn empty_pages_block_does_not_emit_exec_line() {
        let cfg = cfg_with(vec![], "F1");
        let out = update_autoexec("", &cfg);
        assert!(out.contains(AUTOEXEC_BEGIN));
        assert!(!out.contains("exec bindmgr_page"));
    }

    #[test]
    fn block_does_not_emit_cheat_protected_commands() {
        // screenmessage_show is cheat-protected on Valve servers; including
        // it in autoexec would just spam an "ignored" warning into the
        // console on every game launch.
        let out = update_autoexec("", &cfg());
        assert!(!out.contains("screenmessage_show"));
    }
}
