//! Windows font fallbacks that extend egui's defaults to cover the Unicode
//! ranges CS2 players actually paste into chat binds.
//!
//! egui's bundled `default_fonts` (Ubuntu-Light + Hack + NotoEmoji) cover
//! plain Latin, common emoji, and a sliver of symbols, but they're blank
//! on many characters CS2 can render, including **Mathematical Alphanumeric
//! Symbols** (U+1D400–U+1D7FF, e.g. the double-struck letters `𝕊𝕔𝕠𝕠𝕡𝕤`),
//! **Hangul Jamo** (U+1100–U+11FF, e.g. `ᆺ`), and some modifier letters
//! (e.g. `˃` / `˂`). Without coverage those glyphs render as tofu in the bind
//! editor and the import dialog.
//!
//! The app is Windows-only for now, so we append built-in Windows fonts at the
//! end of every font family's fallback list instead of bundling large font
//! files in the repository. egui only reaches for them when the primary fonts
//! have no glyph for a given codepoint.

use std::{fs, path::PathBuf, sync::Arc};

const WINDOWS_FONT_FALLBACKS: &[WindowsFontFallback] = &[
    WindowsFontFallback {
        name: "windows_segoe_ui",
        file: "segoeui.ttf",
    },
    WindowsFontFallback {
        name: "windows_segoe_ui_symbol",
        file: "seguisym.ttf",
    },
    WindowsFontFallback {
        name: "windows_malgun_gothic",
        file: "malgun.ttf",
    },
];

struct WindowsFontFallback {
    name: &'static str,
    file: &'static str,
}

/// Wires Windows system fallback fonts into the given egui context.
/// Call this once from the eframe creation hook.
pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    for fallback in WINDOWS_FONT_FALLBACKS {
        let Ok(data) = fs::read(windows_font_path(fallback.file)) else {
            continue;
        };

        fonts.font_data.insert(
            fallback.name.into(),
            Arc::new(egui::FontData::from_owned(data)),
        );

        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push(fallback.name.into());
        }
    }

    ctx.set_fonts(fonts);
}

fn windows_font_path(file: &str) -> PathBuf {
    std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join("Fonts")
        .join(file)
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::{windows_font_path, WINDOWS_FONT_FALLBACKS};

    #[test]
    fn windows_fallbacks_cover_known_cs2_chat_characters() {
        let fonts = WINDOWS_FONT_FALLBACKS
            .iter()
            .map(|fallback| {
                std::fs::read(windows_font_path(fallback.file)).unwrap_or_else(|err| {
                    panic!("failed to read Windows font {}: {err}", fallback.file)
                })
            })
            .collect::<Vec<_>>();

        for ch in "˃ᆺ˂𝕊𝕔𝕠𝕠𝕡𝕤".chars() {
            assert!(
                fonts.iter().any(|font| has_glyph(font, ch)),
                "Windows font fallbacks must cover {ch:?} (U+{:04X})",
                ch as u32
            );
        }
    }

    fn has_glyph(font: &[u8], ch: char) -> bool {
        ttf_parser::Face::parse(font, 0).is_ok_and(|face| face.glyph_index(ch).is_some())
    }
}
