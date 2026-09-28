//! Light or dark (BISE-02, book §5): pick the palette from the terminal's
//! background.
//!
//! At start, in raw mode and before the alternate screen, we ask the
//! terminal for its background color (OSC 11) and, right after, for its
//! device attributes (DA1). Every terminal answers DA1, so its reply marks
//! the end of the answers: no need to wait out the timeout when OSC 11 is
//! not supported. No answer within ~100 ms (tmux without passthrough, a
//! dumb terminal): dark.
//!
//! `BISE_THEME=light|dark|auto` forces a mode (auto, or anything else,
//! detects). `/theme` calls [`choose`]: it applies and saves.
//!
//! BISE-62: the choice is saved in `~/.bend-harness/tui.json` (`"theme":
//! "light" | "dark" | "auto"`, next to `/voice`'s key) and reused at the
//! next launch. Precedence: `BISE_THEME` > the saved choice > detection.

use crate::theme::{self, Mode};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Once;
use std::time::Duration;

/// The env var that forces the mode.
pub(crate) const ENV: &str = "BISE_THEME";

/// How long we wait for the terminal's answers.
const TIMEOUT: Duration = Duration::from_millis(100);

/// What the user asks for: a mode, or the terminal's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Choice {
    Auto,
    Light,
    Dark,
}

impl Choice {
    /// `light`, `dark` or `auto` (any case, trimmed); anything else: None.
    pub(crate) fn parse(s: &str) -> Option<Choice> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Choice::Auto),
            "light" => Some(Choice::Light),
            "dark" => Some(Choice::Dark),
            _ => None,
        }
    }
}

// what the terminal told us at start: 0 = not asked / no answer, 1 = dark,
// 2 = light
static DETECTED: AtomicU8 = AtomicU8::new(0);

/// The mode read from the terminal's background at start, if it answered.
pub(crate) fn detected() -> Option<Mode> {
    match DETECTED.load(Ordering::Relaxed) {
        1 => Some(Mode::Dark),
        2 => Some(Mode::Light),
        _ => None,
    }
}

/// Switch to `choice` and return the mode now in use. `Auto` uses the
/// background read at start (we never query while the UI reads keys: the
/// answer would arrive as keystrokes); no answer then: dark.
pub(crate) fn apply(choice: Choice) -> Mode {
    let m = match choice {
        Choice::Light => Mode::Light,
        Choice::Dark => Mode::Dark,
        Choice::Auto => detected().unwrap_or(Mode::Dark),
    };
    theme::set_mode(m);
    m
}

// ---- the saved choice (BISE-62) ----

impl Choice {
    fn word(self) -> &'static str {
        match self {
            Choice::Auto => "auto",
            Choice::Light => "light",
            Choice::Dark => "dark",
        }
    }
}

/// `~/.bend-harness/tui.json` under `home` (the file `/voice` saves in).
pub(crate) fn settings_path(home: &std::path::Path) -> std::path::PathBuf {
    home.join(".bend-harness").join("tui.json")
}

/// The `theme` of a settings text, if any.
pub(crate) fn saved_from(settings: &str) -> Option<Choice> {
    let v: serde_json::Value = serde_json::from_str(settings).ok()?;
    Choice::parse(v.get("theme")?.as_str()?)
}

/// The settings text with `theme` set, the other keys kept.
pub(crate) fn with_theme(settings: Option<&str>, choice: Choice) -> String {
    let mut v = settings
        .and_then(|t| serde_json::from_str::<serde_json::Value>(t).ok())
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));
    v["theme"] = serde_json::Value::String(choice.word().into());
    serde_json::to_string_pretty(&v).unwrap_or_default() + "\n"
}

/// The choice saved under `home`, if any.
pub(crate) fn load_in(home: &std::path::Path) -> Option<Choice> {
    saved_from(&std::fs::read_to_string(settings_path(home)).ok()?)
}

/// Save `choice` under `home`.
pub(crate) fn save_in(home: &std::path::Path, choice: Choice) -> Result<(), String> {
    let path = settings_path(home);
    let old = std::fs::read_to_string(&path).ok();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, with_theme(old.as_deref(), choice)).map_err(|e| e.to_string())
}

fn home() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").filter(|h| !h.is_empty()).map(std::path::PathBuf::from)
}

/// `/theme light|dark|auto`: switch now and save it for the next launches.
/// The mode is switched even when the save fails (the error says why).
#[allow(dead_code)] // until /theme (BISE-41, track K) calls it
pub(crate) fn choose(choice: Choice) -> (Mode, Result<(), String>) {
    let m = apply(choice);
    let saved = match home() {
        Some(h) => save_in(&h, choice),
        None => Err("HOME is not set".into()),
    };
    (m, saved)
}

/// What the launch uses: `BISE_THEME`, else the saved choice, else auto.
pub(crate) fn startup_choice(env: Option<&str>, saved: Option<Choice>) -> Choice {
    env.and_then(Choice::parse).or(saved).unwrap_or(Choice::Auto)
}

/// Once per process, from the terminal init (raw mode on, alternate screen
/// not yet entered): `BISE_THEME`, else the saved choice; ask the terminal
/// when it is auto, set the mode. Later inits (after a suspend) keep what
/// is set, so a `/theme` choice survives them.
pub(crate) fn init() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let choice = startup_choice(
            std::env::var(ENV).ok().as_deref(),
            home().and_then(|h| load_in(&h)),
        );
        if choice == Choice::Auto {
            if let Some(rgb) = tty::query_background(TIMEOUT) {
                let m = mode_for(rgb);
                DETECTED.store(if m == Mode::Light { 2 } else { 1 }, Ordering::Relaxed);
            }
        }
        apply(choice);
    });
}

/// Light when the background is lighter than mid-grey in perceived
/// lightness (L* > 50, relative luminance > 0.184): the point where our
/// dark text reads better on it than our light text.
pub(crate) fn mode_for(rgb: [f64; 3]) -> Mode {
    if relative_luminance(rgb) > 0.184 {
        Mode::Light
    } else {
        Mode::Dark
    }
}

fn relative_luminance([r, g, b]: [f64; 3]) -> f64 {
    let lin = |c: f64| {
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

/// The background from an OSC 11 reply anywhere in `buf`
/// (`ESC ] 11 ; rgb:RRRR/GGGG/BBBB` then BEL or ESC \), each channel 1 to 4
/// hex digits, scaled to 0..=1. `rgba:` is read too (alpha ignored).
pub(crate) fn parse_osc11(buf: &[u8]) -> Option<[f64; 3]> {
    let text = String::from_utf8_lossy(buf);
    let start = text.find("]11;")? + 4;
    let body = &text[start..];
    let body = body
        .strip_prefix("rgb:")
        .or_else(|| body.strip_prefix("rgba:"))?;
    let end = body.find(['\x07', '\x1b']).unwrap_or(body.len());
    let mut parts = body[..end].split('/');
    let mut rgb = [0.0; 3];
    for c in rgb.iter_mut() {
        *c = channel(parts.next()?)?;
    }
    Some(rgb)
}

fn channel(hex: &str) -> Option<f64> {
    if hex.is_empty() || hex.len() > 4 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(hex, 16).ok()?;
    let max = (1u32 << (4 * hex.len())) - 1;
    Some(v as f64 / max as f64)
}

/// True once `buf` holds a DA1 reply (`ESC [ ? … c`): the terminal has
/// answered everything we asked.
pub(crate) fn has_da1(buf: &[u8]) -> bool {
    let mut i = 0;
    while i + 2 < buf.len() {
        if buf[i] == 0x1b && buf[i + 1] == b'[' && buf[i + 2] == b'?' {
            let rest = &buf[i + 3..];
            if let Some(n) = rest.iter().position(|b| !(b.is_ascii_digit() || *b == b';')) {
                if rest[n] == b'c' {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

#[cfg(unix)]
mod tty {
    //! The query itself, on /dev/tty. `std` has no way to wait on a fd
    //! with a timeout: we declare libc's `select` (always linked on unix)
    //! instead of adding a dependency. Not `poll`: on macOS it does not
    //! support ttys (it returns at once with POLLNVAL).
    use std::fs::OpenOptions;
    use std::io::{IsTerminal, Read, Write};
    use std::os::fd::AsRawFd;
    use std::time::{Duration, Instant};

    /// `fd_set`: 1024 bits on macOS and Linux (bit n = byte n/8, bit n%8
    /// on little-endian; the words are 32 or 64 bits wide, same layout).
    #[repr(C, align(8))]
    struct FdSet([u8; 128]);

    #[repr(C)]
    struct TimeVal {
        tv_sec: i64,
        #[cfg(target_os = "macos")]
        tv_usec: i32,
        #[cfg(not(target_os = "macos"))]
        tv_usec: i64,
    }

    extern "C" {
        fn select(
            nfds: std::ffi::c_int,
            readfds: *mut FdSet,
            writefds: *mut FdSet,
            errorfds: *mut FdSet,
            timeout: *mut TimeVal,
        ) -> std::ffi::c_int;
    }

    fn readable(fd: std::ffi::c_int, wait: Duration) -> bool {
        if !(0..1024).contains(&fd) || cfg!(target_endian = "big") {
            return false;
        }
        let mut set = FdSet([0; 128]);
        set.0[fd as usize / 8] |= 1 << (fd % 8);
        let mut tv = TimeVal { tv_sec: wait.as_secs() as i64, tv_usec: wait.subsec_micros() as _ };
        // SAFETY: a valid fd below FD_SETSIZE in a zeroed set, null write
        // and error sets, a valid timeout
        let n = unsafe {
            select(fd + 1, &mut set, std::ptr::null_mut(), std::ptr::null_mut(), &mut tv)
        };
        n > 0 && set.0[fd as usize / 8] & (1 << (fd % 8)) != 0
    }

    /// Ask OSC 11 then DA1; read until the DA1 reply or the timeout.
    pub(super) fn query_background(timeout: Duration) -> Option<[f64; 3]> {
        super::parse_osc11(&query_raw(timeout)?)
    }

    /// The raw answers (the probe test prints them).
    pub(super) fn query_raw(timeout: Duration) -> Option<Vec<u8>> {
        if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
            return None;
        }
        let mut tty = OpenOptions::new().read(true).write(true).open("/dev/tty").ok()?;
        tty.write_all(b"\x1b]11;?\x07\x1b[c").ok()?;
        tty.flush().ok()?;
        let fd = tty.as_raw_fd();
        let deadline = Instant::now() + timeout;
        let mut buf = Vec::with_capacity(64);
        let mut chunk = [0u8; 256];
        while !super::has_da1(&buf) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() || !readable(fd, left) {
                break;
            }
            match tty.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
            }
        }
        Some(buf)
    }
}

#[cfg(not(unix))]
mod tty {
    pub(super) fn query_background(_timeout: std::time::Duration) -> Option<[f64; 3]> {
        None
    }
    pub(super) fn query_raw(_timeout: std::time::Duration) -> Option<Vec<u8>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f64; 3], b: [f64; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3)
    }

    #[test]
    fn parses_four_digit_white() {
        let rgb = parse_osc11(b"\x1b]11;rgb:ffff/ffff/ffff\x07").unwrap();
        assert!(close(rgb, [1.0, 1.0, 1.0]));
        assert_eq!(mode_for(rgb), Mode::Light);
    }

    #[test]
    fn parses_two_digit_and_st_terminator() {
        let rgb = parse_osc11(b"\x1b]11;rgb:28/2c/34\x1b\\").unwrap();
        assert!(close(rgb, [0x28 as f64 / 255.0, 0x2c as f64 / 255.0, 0x34 as f64 / 255.0]));
        assert_eq!(mode_for(rgb), Mode::Dark);
    }

    #[test]
    fn parses_four_digit_cream_and_one_and_three_digits() {
        let rgb = parse_osc11(b"\x1b]11;rgb:f7f7/f4f4/eeee\x07").unwrap();
        assert_eq!(mode_for(rgb), Mode::Light);
        assert!(close(parse_osc11(b"\x1b]11;rgb:f/0/8\x07").unwrap(), [1.0, 0.0, 8.0 / 15.0]));
        assert!(close(parse_osc11(b"\x1b]11;rgb:fff/000/fff\x07").unwrap(), [1.0, 0.0, 1.0]));
    }

    #[test]
    fn finds_the_reply_among_other_bytes() {
        let buf = b"x\x1b]11;rgba:0000/0000/0000/ffff\x07\x1b[?62;22c";
        assert!(close(parse_osc11(buf).unwrap(), [0.0, 0.0, 0.0]));
        assert!(has_da1(buf));
    }

    #[test]
    fn garbage_is_none() {
        for g in [
            &b""[..],
            b"garbage",
            b"\x1b]11;?\x07",
            b"\x1b]11;rgb:zz/00/00\x07",
            b"\x1b]11;rgb:12345/0/0\x07",
            b"\x1b]11;rgb:ff/ff\x07",
            b"\x1b]11;rgb://\x07",
            b"\x1b]10;rgb:ff/ff/ff\x07",
            b"\x1b]11;#ffffff\x07",
        ] {
            assert_eq!(parse_osc11(g), None, "{:?}", String::from_utf8_lossy(g));
        }
    }

    #[test]
    fn da1_marks_the_end() {
        assert!(has_da1(b"\x1b[?1;2c"));
        assert!(has_da1(b"\x1b[?65;1;9c"));
        assert!(!has_da1(b"\x1b[?1;2"));
        assert!(!has_da1(b"\x1b]11;rgb:0/0/0\x07"));
    }

    #[test]
    fn mid_grey_threshold() {
        // #808080 (L* 54) reads better with dark text; #707070 (L* 48) with light
        assert_eq!(mode_for([0x80 as f64 / 255.0; 3]), Mode::Light);
        assert_eq!(mode_for([0x70 as f64 / 255.0; 3]), Mode::Dark);
        assert_eq!(mode_for([0x14 as f64 / 255.0, 0x12 as f64 / 255.0, 0x11 as f64 / 255.0]), Mode::Dark);
    }

    #[test]
    fn the_choice_is_saved_next_to_the_other_settings() {
        let h = std::env::temp_dir().join(format!("bise-theme-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&h);
        assert_eq!(load_in(&h), None);
        std::fs::create_dir_all(h.join(".bend-harness")).unwrap();
        std::fs::write(settings_path(&h), "{\"voice_mode_enabled\": true}").unwrap();
        save_in(&h, Choice::Light).unwrap();
        assert_eq!(load_in(&h), Some(Choice::Light));
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(settings_path(&h)).unwrap()).unwrap();
        assert_eq!(v["voice_mode_enabled"], serde_json::Value::Bool(true));
        save_in(&h, Choice::Auto).unwrap();
        assert_eq!(load_in(&h), Some(Choice::Auto));
        assert_eq!(saved_from("{\"theme\": \"pink\"}"), None);
        assert_eq!(saved_from("not json"), None);
        let _ = std::fs::remove_dir_all(&h);
    }

    #[test]
    fn env_beats_the_saved_choice_beats_detection() {
        assert_eq!(startup_choice(Some("dark"), Some(Choice::Light)), Choice::Dark);
        assert_eq!(startup_choice(None, Some(Choice::Light)), Choice::Light);
        assert_eq!(startup_choice(Some("junk"), Some(Choice::Light)), Choice::Light);
        assert_eq!(startup_choice(Some("auto"), Some(Choice::Light)), Choice::Auto);
        assert_eq!(startup_choice(None, None), Choice::Auto);
    }

    #[test]
    fn choice_parses() {
        assert_eq!(Choice::parse(" Light "), Some(Choice::Light));
        assert_eq!(Choice::parse("DARK"), Some(Choice::Dark));
        assert_eq!(Choice::parse("auto"), Some(Choice::Auto));
        assert_eq!(Choice::parse("blue"), None);
    }

    /// By hand, in a real terminal (writes /tmp/bise-theme-probe.txt):
    /// `cargo test -p bend-tui --lib probe_this_terminal -- --ignored`
    #[test]
    #[ignore]
    fn probe_this_terminal() {
        crossterm::terminal::enable_raw_mode().unwrap();
        let t0 = std::time::Instant::now();
        let raw = tty::query_raw(Duration::from_millis(300));
        let took = t0.elapsed();
        let rgb = raw.as_deref().and_then(parse_osc11);
        crossterm::terminal::disable_raw_mode().unwrap();
        let line = format!(
            "TERM_PROGRAM={:?} TMUX={} raw={:?} rgb={rgb:?} mode={:?} took={took:?}\n",
            std::env::var("TERM_PROGRAM").ok(),
            std::env::var("TMUX").is_ok(),
            raw.map(|r| String::from_utf8_lossy(&r).into_owned()),
            rgb.map(mode_for)
        );
        std::fs::write("/tmp/bise-theme-probe.txt", line).unwrap();
    }

    #[test]
    fn apply_switches_the_mode() {
        assert_eq!(apply(Choice::Light), Mode::Light);
        assert_eq!(theme::mode(), Mode::Light);
        assert_eq!(apply(Choice::Dark), Mode::Dark);
        assert_eq!(theme::mode(), Mode::Dark);
        // nothing detected in tests: auto falls back to dark
        assert_eq!(apply(Choice::Auto), detected().unwrap_or(Mode::Dark));
    }
}
