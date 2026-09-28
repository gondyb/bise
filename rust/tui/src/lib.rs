//! bend-tui — Ratatui terminal UI for the Bend Unified Harness.
//!
//! Rust port of repl-tui (Ink). Same wire protocol; the presentation is
//! modeled on the REAL OpenCode TUI (packages/tui in the opencode repo):
//! no header bar — the screen is the conversation. Blocks breathe: a
//! blank line at every content transition, one column of margin on
//! each edge of the feed, blank rows separating the history from the
//! composer, and the user block paints its panel background the full
//! column. Status speaks in glyphs, not words: ✦ reasoning, ✓ ok,
//! ✗ fail, ▲ warning, ⟳ compaction, ≡ summary, ↳ preview. User
//! messages are
//! blocks with a colored left bar and a panel background; assistant
//! markdown renders in the OpenCode markdown colors; tool calls are
//! OpenCode inline tools (braille spinner while running, muted ✓ once
//! done, red ✗ on failure); the prompt is an OpenCode prompt (left
//! border, element background, agent/model meta row); commands filter
//! in an OpenCode autocomplete popup (split border, primary selection).
//! The status row carries the spinner + ctrl+c-to-interrupt hints.
//!
//! When stdin/stdout is not a TTY (piped), it falls back to line mode so
//! the UI stays scriptable — the same convention as the Ink version.

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Paragraph,
};
use ratatui::Frame;
use std::io::{self, IsTerminal, Write};
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

mod theme;
use theme::*;
mod wire;
use wire::*;
mod markdown;
use markdown::*;
mod code;
mod render;
use render::*;
mod feed;
use feed::*;
mod app;
use app::*;
mod commands;
pub use commands::HarnessInfo;
use commands::*;
mod ui;
use ui::*;
mod input;
use input::*;
mod run;
pub use run::run;
use run::*;
mod sb;
mod skills;
mod files;
mod emoji;
mod editor;
mod clipboard;
mod usage;
mod term;
mod feedsel;
mod keyprobe;
mod help;
mod voice;
#[cfg(test)]
mod voice_ui_tests;
#[cfg(test)]
mod composer_wrap_tests;
#[cfg(test)]
mod feed_render_tests;
pub use keyprobe::keyprobe;
pub use sb::{run_switchboard, take_reexec};

