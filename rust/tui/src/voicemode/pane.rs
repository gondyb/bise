//! Voice mode's pane in place of the composer (owner: voice-tui; design
//! §0, plan §2): from 30 rows the kiss + captions + status row, half
//! the screen; under 30 rows B's two lanes (you, the agent) in 4 rows.
//! Stub: filled by voice-tui.

use super::PaneView;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// The pane's rows for a screen `screen_h` rows high.
pub fn height(screen_h: u16) -> u16 {
    if screen_h < 30 {
        4
    } else {
        screen_h / 2
    }
}

/// Draws `v` into `area` at the animation time `t_ms`.
pub fn draw(_buf: &mut Buffer, _area: Rect, _v: &PaneView, _t_ms: u64) {}
