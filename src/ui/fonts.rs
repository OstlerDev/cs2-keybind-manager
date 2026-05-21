//! Font fallbacks that extend egui's defaults to cover the Unicode ranges
//! CS2 players actually paste into chat binds.
//!
//! egui's bundled `default_fonts` (Ubuntu-Light + Hack + NotoEmoji) cover
//! plain Latin, common emoji, and a sliver of symbols, but they're blank
//! on **Mathematical Alphanumeric Symbols** (U+1D400–U+1D7FF, e.g. the
//! double-struck letters `𝕊𝕔𝕠𝕠𝕡𝕤`) and most of the **Dingbats** block
//! (U+2700–U+27BF, e.g. the heavy floral heart `✿`). Without coverage those
//! glyphs render as tofu in the bind editor and the import dialog.
//!
//! We append two SIL-OFL-licensed Noto fonts at the **end** of every font
//! family's fallback list, so egui only reaches for them when the primary
//! fonts have no glyph for a given codepoint. See `assets/fonts/NOTICE.md`
//! for license details.

use std::sync::Arc;

const NOTO_MATH: &[u8] = include_bytes!("../../assets/fonts/NotoSansMath-Regular.ttf");
const NOTO_SYMBOLS2: &[u8] = include_bytes!("../../assets/fonts/NotoSansSymbols2-Regular.ttf");

/// Wires the bundled Noto fallback fonts into the given egui context.
/// Call this once from the eframe creation hook.
pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "noto_math".into(),
        Arc::new(egui::FontData::from_static(NOTO_MATH)),
    );
    fonts.font_data.insert(
        "noto_symbols2".into(),
        Arc::new(egui::FontData::from_static(NOTO_SYMBOLS2)),
    );

    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let chain = fonts.families.entry(family).or_default();
        chain.push("noto_math".into());
        chain.push("noto_symbols2".into());
    }

    ctx.set_fonts(fonts);
}
