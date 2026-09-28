//! The OpenCode dark theme (opencode.json): colors, borders, the spinner
//! and the feed glyph vocabulary (status is symbols, not words).
//! Primary #fab283 (the OpenCode orange) is the agent color: user blocks,
//! prompt border, spinners, back-to-bottom. Markdown follows the
//! markdown* keys; tools follow the inline-tool rules (muted once
//! complete, error red).

use ratatui::style::Color;
use ratatui::symbols::border;

pub(crate) const BRAND: Color = Color::Rgb(0xfa, 0xb2, 0x83); // primary
pub(crate) const ACCENT: Color = Color::Rgb(0x9d, 0x7c, 0xd8); // markdownHeading
pub(crate) const HEAD: Color = Color::Rgb(0xe5, 0xc0, 0x7b); // markdownEmph / syntaxType
pub(crate) const INFO: Color = Color::Rgb(0x56, 0xb6, 0xc2); // info / markdownListEnumeration
pub(crate) const TEXT: Color = Color::Rgb(0xee, 0xee, 0xee); // text
pub(crate) const DIM: Color = Color::Rgb(0x80, 0x80, 0x80); // textMuted
                                                 // complete tools sit at textMuted (OpenCode: fg textMuted when complete)
pub(crate) const TOOL: Color = Color::Rgb(0x80, 0x80, 0x80);
pub(crate) const OK: Color = Color::Rgb(0x7f, 0xd8, 0x8f); // success / markdownCode
pub(crate) const WARN: Color = Color::Rgb(0xf5, 0xa7, 0x42); // warning / markdownStrong
pub(crate) const ERR: Color = Color::Rgb(0xe0, 0x6c, 0x75); // error
pub(crate) const PANEL: Color = Color::Rgb(0x14, 0x14, 0x14); // backgroundPanel
/// text on a BRAND background (the popup selection): black reads
/// better than white on orange
pub(crate) const ON_BRAND: Color = Color::Rgb(0, 0, 0);
pub(crate) const ELEMENT: Color = Color::Rgb(0x1e, 0x1e, 0x1e); // backgroundElement
/// the composer while recording (Vibe's mistral_orange)
pub(crate) const RECORDING: Color = Color::Rgb(0xff, 0x82, 0x05);
pub(crate) const SELECTION: Color = Color::Rgb(0x3a, 0x4a, 0x6b); // selected text background
pub(crate) const BORDER_ACTIVE: Color = Color::Rgb(0x60, 0x60, 0x60); // borderActive
pub(crate) const FAINT: Color = Color::Rgb(0x4a, 0x4a, 0x4a); // rails & turn marks — dimmer than textMuted
// the syntax palette of the run_typescript block (OpenCode dark
// syntax colors: purple keywords, green strings, faint comments,
// orange numbers, blue calls; types reuse syntaxType)
pub(crate) const SYNTAX_KEYWORD: Color = Color::Rgb(0xc6, 0x78, 0xdd);
pub(crate) const SYNTAX_STRING: Color = Color::Rgb(0x98, 0xc3, 0x79);
pub(crate) const SYNTAX_COMMENT: Color = Color::Rgb(0x5c, 0x63, 0x70);
pub(crate) const SYNTAX_NUMBER: Color = Color::Rgb(0xd1, 0x9a, 0x66);
pub(crate) const SYNTAX_FUNC: Color = Color::Rgb(0x61, 0xaf, 0xef);
// the apply_patch diff bands (dark tints under the ok / error colors)
pub(crate) const DIFF_ADD_BG: Color = Color::Rgb(0x16, 0x2e, 0x1c);
pub(crate) const DIFF_DEL_BG: Color = Color::Rgb(0x3a, 0x18, 0x1b);

// the OpenCode prompt/autocomplete borders: only a colored vertical bar
pub(crate) const SPLIT: border::Set = border::Set {
    top_left: "",
    top_right: "",
    bottom_left: "",
    bottom_right: "",
    vertical_left: "┃",
    vertical_right: "┃",
    horizontal_top: " ",
    horizontal_bottom: " ",
};

// the OpenCode spinner (component/spinner.tsx): braille dots
pub(crate) const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub(crate) fn spinner_frame(tick: u32) -> &'static str {
    SPINNER[(tick as usize) % SPINNER.len()]
}

// ---- the feed glyph vocabulary: status is symbols, not words ----
pub(crate) const GLYPH_THINK: &str = "✦"; // reasoning section (duration when collapsed)
pub(crate) const GLYPH_OK: &str = "✓"; // success (tool, sub-call, turn)
pub(crate) const GLYPH_ERR: &str = "✗"; // failure
pub(crate) const GLYPH_WARN: &str = "▲"; // warning (interrupt, discarded candidate)
pub(crate) const GLYPH_INFO: &str = "·"; // neutral notice
pub(crate) const GLYPH_COMPACT: &str = "⟳"; // compaction running
pub(crate) const GLYPH_SUMMARY: &str = "≡"; // compaction summary
pub(crate) const GLYPH_BRANCH: &str = "↳"; // preview / sub-result line
pub(crate) const GLYPH_RAIL: &str = "│"; // rail of an expanded reasoning section
