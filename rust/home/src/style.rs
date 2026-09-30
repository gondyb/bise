//! The look of bise's commands for humans (BISE-285): `bise login`,
//! `doctor`, `models`, `auth list`, the help… One small set of pieces so
//! every command reads the same, in the TUI's voice and colors.
//!
//! - **roles, not colors** (the TUI's theme, book §5-6): text (the
//!   terminal's own color), dim, faint, accent (pink: needs you, and the
//!   ✓ of done: no green, the brand's own pink), error (red ✗). The dark or light palette: the theme saved
//!   in the TUI (`/theme`), else `COLORFGBG`, else dark.
//! - **marks**: `✓` done, `✗` failed, `?` needs you (a warning, a step
//!   left to do), the TUI's glyphs.
//! - **blocks**: a title line, key/value rows aligned under it, a blank
//!   line between blocks, and a `next:` line when there is one obvious
//!   next step.
//! - **links**: a URL is an OSC 8 link on a terminal (a click opens it)
//!   and stays the plain URL, so a copy or a terminal without OSC 8 still
//!   gets it.
//!
//! Colors and links only on a terminal: `NO_COLOR` (any value), `TERM=dumb`
//! or a pipe give [`Style::PLAIN`], whose text is the same words with no
//! escape at all: piped output stays stable for scripts.

use std::io::IsTerminal;

/// What a piece of text is (the TUI theme's roles).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// everything you read: the terminal's own foreground
    Text,
    /// secondary text: labels, details
    Dim,
    /// the quietest text: hints, paths
    Faint,
    /// needs you, and done's ✓ (pink)
    Accent,
    /// failed (red)
    Error,
}

/// How a stream is written: plain, or with the TUI's colors and links.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Style {
    /// colors and OSC 8 links
    pub color: bool,
    /// the light palette (a light terminal)
    pub light: bool,
    /// the terminal's columns, to wrap long values; 0 = never wrap (a pipe)
    pub width: usize,
}

/// The TUI's palettes (rust/tui/src/theme.rs), the roles a command uses:
/// dim, faint, accent, error.
const DARK: [u32; 4] = [0xa39c90, 0x857d72, 0xf4a6b0, 0xff5a52];
const LIGHT: [u32; 4] = [0x6b645a, 0x7d766c, 0xb8416b, 0xb3261e];

/// The marks.
pub const OK: &str = "✓";
pub const FAIL: &str = "✗";
pub const ASK: &str = "?";

impl Style {
    /// No escape at all: a pipe, a file, a test.
    pub const PLAIN: Style = Style { color: false, light: false, width: 0 };

    /// For stdout / stderr of this process.
    pub fn stdout() -> Style {
        let tty = std::io::stdout().is_terminal();
        Style { width: if tty { columns(1) } else { 0 }, ..Style::detect(tty, &env, saved_theme().as_deref()) }
    }
    pub fn stderr() -> Style {
        let tty = std::io::stderr().is_terminal();
        Style { width: if tty { columns(2) } else { 0 }, ..Style::detect(tty, &env, saved_theme().as_deref()) }
    }

    /// `tty`: the stream is a terminal; `theme`: the TUI's saved choice
    /// ("light", "dark", "auto").
    pub fn detect(tty: bool, env: &dyn Fn(&str) -> Option<String>, theme: Option<&str>) -> Style {
        let no_color = env("NO_COLOR").is_some() || env("TERM").as_deref() == Some("dumb");
        let light = match theme {
            Some("light") => true,
            Some("dark") => false,
            // "15;0": foreground 15 on background 0; 7 and 15 are light grounds
            _ => env("COLORFGBG").and_then(|v| v.rsplit(';').next().map(|b| matches!(b.trim(), "7" | "15"))).unwrap_or(false),
        };
        Style { color: tty && !no_color, light, width: 0 }
    }

    /// `s` in `role`.
    pub fn paint(&self, role: Role, s: &str) -> String {
        if !self.color || s.is_empty() {
            return s.to_string();
        }
        let p = if self.light { &LIGHT } else { &DARK };
        let hex = match role {
            Role::Text => return s.to_string(),
            Role::Dim => p[0],
            Role::Faint => p[1],
            Role::Accent => p[2],
            Role::Error => p[3],
        };
        format!("\x1b[38;2;{};{};{}m{}\x1b[39m", (hex >> 16) & 0xff, (hex >> 8) & 0xff, hex & 0xff, s)
    }

    pub fn dim(&self, s: &str) -> String {
        self.paint(Role::Dim, s)
    }
    pub fn faint(&self, s: &str) -> String {
        self.paint(Role::Faint, s)
    }
    pub fn accent(&self, s: &str) -> String {
        self.paint(Role::Accent, s)
    }

    /// A title: bold on a terminal.
    pub fn title(&self, s: &str) -> String {
        if self.color {
            format!("\x1b[1m{}\x1b[22m", s)
        } else {
            s.to_string()
        }
    }

    /// A URL: an OSC 8 link on a terminal, its text the URL itself.
    pub fn link(&self, url: &str) -> String {
        if !self.color || url.is_empty() {
            return url.to_string();
        }
        format!("\x1b]8;;{url}\x1b\\\x1b[4m{url}\x1b[24m\x1b]8;;\x1b\\")
    }

    /// "✓ msg": done (the pink ✓, as in the TUI).
    pub fn ok(&self, msg: &str) -> String {
        format!("{} {}", self.paint(Role::Accent, OK), msg)
    }
    /// "✗ msg": failed.
    pub fn fail(&self, msg: &str) -> String {
        format!("{} {}", self.paint(Role::Error, FAIL), msg)
    }
    /// "? msg": needs you (a warning, something left to do).
    pub fn ask(&self, msg: &str) -> String {
        format!("{} {}", self.paint(Role::Accent, ASK), msg)
    }

    /// "next: what" (the label dim).
    pub fn next(&self, what: &str) -> String {
        format!("{} {}", self.dim("next:"), what)
    }

    /// Key/value rows, the keys dim and padded to the longest: one line
    /// each, `indent` spaces first. A value on several lines keeps its
    /// continuation lines under the value's column.
    pub fn rows(&self, indent: usize, rows: &[(&str, String)]) -> String {
        let w = rows.iter().map(|(k, _)| width(k)).max().unwrap_or(0);
        let mut o = String::new();
        for (k, v) in rows {
            let pad = " ".repeat(w - width(k));
            let mut lines = v.lines();
            let first = lines.next().unwrap_or("");
            let head = format!("{}{}{}  {}", " ".repeat(indent), self.dim(k), pad, first);
            o.push_str(head.trim_end());
            o.push('\n');
            for l in lines {
                o.push_str(&format!("{}{}\n", " ".repeat(indent + w + 2), l));
            }
        }
        o
    }
}

/// `text` cut at spaces into lines of at most `width` chars (a word
/// longer than that gets a line of its own); `width` 0: one line.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 || text.chars().count() <= width {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut cur = String::new();
    for w in text.split(' ') {
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > width {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(w);
    }
    lines.push(cur);
    lines
}

/// `text` wrapped to the columns left after `indent`, its next lines
/// indented by `indent` (a hanging indent on the value column): one
/// string, no trailing newline.
pub fn hang(text: &str, indent: usize, width: usize) -> String {
    let room = if width == 0 { 0 } else { width.saturating_sub(indent).max(20) };
    wrap(text, room).join(&format!("\n{}", " ".repeat(indent)))
}

/// The columns of the terminal on `fd`, 0 when unknown.
fn columns(fd: i32) -> usize {
    // SAFETY: TIOCGWINSZ fills a zeroed winsize; nothing else is touched
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    if unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws) } == 0 {
        ws.ws_col as usize
    } else {
        0
    }
}

/// The width of `s` on a terminal, without its escapes (every char one
/// column: the rows' keys are words).
pub fn width(s: &str) -> usize {
    strip(s).chars().count()
}

/// `s` without its SGR and OSC 8 escapes: what a pipe would get.
pub fn strip(s: &str) -> String {
    let mut o = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\x1b' {
            o.push(c);
            continue;
        }
        match it.next() {
            // CSI: up to its final byte
            Some('[') => {
                for c in it.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            // OSC: up to ST (ESC \) or BEL
            Some(']') => {
                while let Some(c) = it.next() {
                    if c == '\x07' {
                        break;
                    }
                    if c == '\x1b' && it.peek() == Some(&'\\') {
                        it.next();
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    o
}

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok()
}

/// The TUI's saved theme ("auto" | "light" | "dark"), if any.
fn saved_theme() -> Option<String> {
    let home = crate::Home::from_env();
    home.pref(crate::Pref::Theme).get().and_then(|v| v.as_str().map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
    }

    const TTY: Style = Style { color: true, light: false, width: 0 };

    #[test]
    fn colors_only_on_a_terminal_without_no_color() {
        let none = env_of(&[]);
        assert!(Style::detect(true, &none, None).color);
        assert!(!Style::detect(false, &none, None).color, "a pipe");
        assert!(!Style::detect(true, &env_of(&[("NO_COLOR", "")]), None).color, "NO_COLOR, even empty");
        assert!(!Style::detect(true, &env_of(&[("NO_COLOR", "1")]), None).color);
        assert!(!Style::detect(true, &env_of(&[("TERM", "dumb")]), None).color);
    }

    #[test]
    fn the_palette_follows_the_saved_theme_then_colorfgbg() {
        let light_bg = env_of(&[("COLORFGBG", "0;15")]);
        assert!(Style::detect(true, &light_bg, None).light);
        assert!(Style::detect(true, &light_bg, Some("auto")).light);
        assert!(!Style::detect(true, &light_bg, Some("dark")).light, "the saved choice wins");
        assert!(Style::detect(true, &env_of(&[]), Some("light")).light);
        assert!(!Style::detect(true, &env_of(&[("COLORFGBG", "15;0")]), None).light);
        assert!(!Style::detect(true, &env_of(&[]), None).light, "no hint: dark");
    }

    #[test]
    fn plain_is_the_same_words_without_any_escape() {
        let p = Style::PLAIN;
        assert_eq!(p.ok("saved"), "✓ saved");
        assert_eq!(p.fail("wrong key"), "✗ wrong key");
        assert_eq!(p.ask("no credit"), "? no credit");
        assert_eq!(p.next("bise"), "next: bise");
        assert_eq!(p.link("https://x.test/keys"), "https://x.test/keys");
        assert_eq!(p.title("bise doctor"), "bise doctor");
        for r in [Role::Text, Role::Dim, Role::Faint, Role::Accent, Role::Error] {
            assert_eq!(p.paint(r, "a"), "a");
        }
        // on a terminal: the same words once the escapes are gone
        for (tty, plain) in [
            (TTY.ok("saved"), p.ok("saved")),
            (TTY.fail("x"), p.fail("x")),
            (TTY.ask("x"), p.ask("x")),
            (TTY.next("bise"), p.next("bise")),
            (TTY.link("https://x.test/keys"), p.link("https://x.test/keys")),
            (TTY.title("t"), p.title("t")),
        ] {
            assert_ne!(tty, plain);
            assert_eq!(strip(&tty), plain);
        }
    }

    #[test]
    fn the_roles_are_the_tui_palette() {
        assert_eq!(TTY.paint(Role::Accent, "?"), "\x1b[38;2;244;166;176m?\x1b[39m");
        // done is the pink ✓, never green
        assert_eq!(TTY.ok("saved"), "\x1b[38;2;244;166;176m✓\x1b[39m saved");
        assert_eq!(TTY.paint(Role::Error, "✗"), "\x1b[38;2;255;90;82m✗\x1b[39m");
        let light = Style { color: true, light: true, width: 0 };
        assert_eq!(light.paint(Role::Accent, "?"), "\x1b[38;2;184;65;107m?\x1b[39m");
        // text is the terminal's own color
        assert_eq!(TTY.paint(Role::Text, "hi"), "hi");
    }

    #[test]
    fn a_link_is_osc8_with_the_url_as_its_text() {
        let l = TTY.link("https://console.mistral.ai/api-keys");
        assert!(l.starts_with("\x1b]8;;https://console.mistral.ai/api-keys\x1b\\"), "{l:?}");
        assert!(l.ends_with("\x1b]8;;\x1b\\"), "{l:?}");
        assert_eq!(strip(&l), "https://console.mistral.ai/api-keys");
    }

    #[test]
    fn rows_align_on_the_longest_key() {
        let rows = [("key", "auth.json".to_string()), ("model", "mistral/x\nand more".to_string()), ("", "".to_string())];
        assert_eq!(Style::PLAIN.rows(2, &rows), "  key    auth.json\n  model  mistral/x\n         and more\n\n");
        // colored: aligned the same once stripped
        assert_eq!(strip(&TTY.rows(2, &rows)), Style::PLAIN.rows(2, &rows));
    }

    #[test]
    fn long_values_wrap_with_a_hanging_indent() {
        assert_eq!(wrap("a b c", 0), vec!["a b c"]);
        assert_eq!(wrap("one two three four", 9), vec!["one two", "three", "four"]);
        assert_eq!(wrap("averyveryverylongword x", 5), vec!["averyveryverylongword", "x"]);
        let h = hang("word ".repeat(12).trim_end(), 10, 40);
        assert!(h.lines().all(|l| l.chars().count() <= 40), "{h}");
        assert!(h.lines().skip(1).all(|l| l.starts_with("          w")), "{h}");
        // a pipe never wraps
        assert_eq!(hang("a b c d", 10, 0), "a b c d");
    }
}
