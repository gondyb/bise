//! Image attachments in the composer (projects/switchboard/docs/images.md).
//!
//! The composer shows `[Image #N]`; the app keeps what each label
//! stands for. On send, every label still in the text becomes its
//! marker (`<image name=… b64=…>`, see the `bend-images` crate) and the
//! attachments start over. Sources: a picked `@` image, a paste that is
//! only image paths (a file dragged into the terminal), Ctrl+V (the
//! clipboard image). Nothing here panics on any input.

use crate::app::App;
use crate::theme::{accent, dim, error, text, G_FAILED, G_IMAGE};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

/// One attached image: its composer label, its marker, what the strip
/// says about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Attachment {
    pub(crate) label: String,
    pub(crate) marker: String,
    pub(crate) info: Info,
}

/// What the strip above the composer says about an attached image.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Info {
    /// where it came from: the path as picked or dropped, or `clipboard`
    pub(crate) source: String,
    /// the size the model gets (after a downscale)
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// the bytes the model gets
    pub(crate) bytes: u64,
    /// downscaled to fit the limits
    pub(crate) resized: bool,
}

/// The label of image `n`.
pub(crate) fn label(n: usize) -> String {
    format!("[Image #{n}]")
}

/// Forget the attachments whose label left the text; the next number.
fn next_number(atts: &mut Vec<Attachment>, text: &str) -> usize {
    atts.retain(|a| text.contains(&a.label));
    let mut n = 1usize;
    while atts.iter().any(|a| a.label == label(n)) {
        n += 1;
    }
    n
}

/// Put `path` in the image store and its label at the cursor; the
/// chip (`▣ 1`) for the flash.
pub(crate) fn attach_file(app: &mut App, path: &Path, source: &str) -> Result<String, String> {
    let stored = bend_images::store_file(path)?;
    let original = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    Ok(add(app, source, source, original, &stored))
}

/// What the strip says about `stored`: the store keeps the bytes as
/// they came unless they were downscaled, so a size change means a
/// resize.
fn info_of(shown: &str, original_bytes: u64, stored: &bend_images::Stored) -> Info {
    let bytes = std::fs::metadata(&stored.file).map(|m| m.len()).unwrap_or(original_bytes);
    Info {
        source: shown.to_string(),
        width: stored.width,
        height: stored.height,
        bytes,
        resized: original_bytes != 0 && bytes != original_bytes,
    }
}

fn add(app: &mut App, source: &str, shown: &str, original: u64, stored: &bend_images::Stored) -> String {
    let n = next_number(&mut app.attachments, &app.ed.text);
    let l = label(n);
    let marker = bend_images::marker(&l, source, stored);
    let info = info_of(shown, original, stored);
    app.attachments.push(Attachment { label: l.clone(), marker, info });
    // a space before the label when it would touch a word
    let before = app.ed.text.chars().nth(app.ed.cursor.wrapping_sub(1));
    let pad = if app.ed.cursor > 0 && before.is_some_and(|c| !c.is_whitespace()) { " " } else { "" };
    app.ed.paste(&format!("{pad}{l} "));
    // the flash names the chip as the composer draws it
    chip_text(&l)
}

/// The clipboard image. Unit tests (the fuzzers press Ctrl+V and paste
/// empty text) never touch the real clipboard: one osascript each would
/// take seconds.
fn clipboard_bytes() -> Result<Vec<u8>, String> {
    if cfg!(test) && std::env::var_os("BEND_CLIPBOARD_IMAGE_FILE").is_none() {
        return Err("no clipboard in unit tests".into());
    }
    bend_images::clipboard_image()
}

/// Ctrl+V: the clipboard image, stored as PNG.
pub(crate) fn attach_clipboard(app: &mut App) -> Result<String, String> {
    let bytes = clipboard_bytes()?;
    let original = bytes.len() as u64;
    let stored = bend_images::store_bytes(bytes)?;
    let source = stored.file.to_string_lossy().to_string();
    Ok(add(app, &source, CLIPBOARD, original, &stored))
}

/// A paste that is only image paths (drag-and-drop): attach them all.
/// None when the paste is something else (it stays text).
pub(crate) fn on_paste(app: &mut App, text: &str) -> Option<Result<Vec<String>, String>> {
    let paths = bend_images::pasted_images(text)?;
    let mut labels = Vec::new();
    for p in &paths {
        match attach_file(app, p, &p.to_string_lossy()) {
            Ok(l) => labels.push(l),
            Err(e) => return Some(Err(e)),
        }
    }
    Some(Ok(labels))
}

/// The text to send: each label still present becomes its marker; the
/// attachments start over.
pub(crate) fn expand(app: &mut App, text: &str) -> String {
    let atts = std::mem::take(&mut app.attachments);
    let mut out = text.to_string();
    for a in &atts {
        out = out.replace(&a.label, &a.marker);
    }
    out
}

/// The picked `@` entry is an image file: attach it (the `@token` goes).
/// Returns the label, or None when it is not an image.
pub(crate) fn pick_image(app: &mut App, rel: &str) -> Option<Result<String, String>> {
    if !bend_images::has_image_ext(rel) {
        return None;
    }
    let (start, _) = crate::files::token(&app.ed.text, app.ed.cursor)?;
    let cursor = app.ed.cursor;
    // the root the `@` popup searched (commands::at_items)
    let root = crate::sb::workspace(app)
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    let path = root.join(rel);
    if !path.is_file() {
        return None;
    }
    // drop the `@token`, then attach at its place
    let chars: Vec<char> = app.ed.text.chars().collect();
    let cursor = cursor.min(chars.len());
    let start = start.min(cursor);
    let head: String = chars.get(..start).unwrap_or(&[]).iter().collect();
    let tail: String = chars.get(cursor..).unwrap_or(&[]).iter().collect();
    let tail = tail.strip_prefix(' ').map(str::to_string).unwrap_or(tail);
    let at = head.chars().count();
    let before = app.ed.text.clone();
    app.ed.set(&format!("{head}{tail}"), at);
    match attach_file(app, &path, rel) {
        Ok(l) => Some(Ok(l)),
        Err(e) => {
            app.ed.set(&before, cursor);
            Some(Err(e))
        }
    }
}

// ---- chips (book §14): `▣ 1` in the composer, `▣ login.png` in the history ----

/// What a clipboard image is called in the strip and the history.
const CLIPBOARD: &str = "clipboard";

/// The labels `[Image #N]` in `text`: (first char index, char index
/// past it, N). The composer draws each as one chip, the cursor steps
/// over it, a delete takes it whole.
pub(crate) fn chips(text: &str) -> Vec<(usize, usize, usize)> {
    const OPEN: &str = "[Image #";
    let mut out = Vec::new();
    let mut from = 0usize; // bytes
    let mut ci = 0usize; // chars before `from`
    while let Some(i) = text.get(from..).and_then(|t| t.find(OPEN)) {
        let at = from + i;
        ci += text[from..at].chars().count();
        let digits: String = text[at + OPEN.len()..].chars().take_while(char::is_ascii_digit).collect();
        let close = at + OPEN.len() + digits.len();
        let n = digits.parse::<usize>().ok().filter(|_| digits.len() <= 6);
        match n {
            Some(n) if text[close..].starts_with(']') => {
                let len = OPEN.len() + digits.len() + 1; // ASCII: bytes = chars
                out.push((ci, ci + len, n));
                from = at + len;
                ci += len;
            }
            _ => {
                from = at + 1;
                ci += 1;
            }
        }
    }
    out
}

/// The chip holding char index `ci` strictly inside (not at its start).
pub(crate) fn chip_around(text: &str, ci: usize) -> Option<(usize, usize)> {
    chips(text).into_iter().find(|&(a, b, _)| a < ci && ci < b).map(|(a, b, _)| (a, b))
}

/// The range `[a, b)` grown to take whole every chip it touches.
pub(crate) fn chip_widen(text: &str, a: usize, b: usize) -> (usize, usize) {
    chips(text).into_iter().fold((a, b), |(a, b), (s, e, _)| {
        if s < b && a < e {
            (a.min(s), b.max(e))
        } else {
            (a, b)
        }
    })
}

/// How the composer draws the label `[Image #N]`.
pub(crate) fn chip_text(label: &str) -> String {
    let n = label.trim_start_matches("[Image #").trim_end_matches(']');
    format!("{G_IMAGE} {n}")
}

/// The style of a chip: accent (the background stays the terminal's).
pub(crate) fn chip_style() -> Style {
    Style::default().fg(accent())
}

/// The image store holds `path` (a clipboard image is stored first,
/// its path is the store's).
#[allow(dead_code)] // the history helpers: render.rs (track F) calls them
fn in_store(path: &str) -> bool {
    let Some(dir) = bend_images::store_dir() else { return false };
    Path::new(path).parent().is_some_and(|p| p == dir)
}

/// The name of an image in the history: `clipboard`, else the file name.
#[allow(dead_code)] // the history helpers: render.rs (track F) calls them
fn short_name(path: &str) -> String {
    if path.is_empty() {
        return "image".into();
    }
    if in_store(path) {
        return CLIPBOARD.into();
    }
    Path::new(path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

thread_local! {
    /// the size of each stored image, by its `.b64` path (read once)
    static SIZES: RefCell<HashMap<String, Option<(u32, u32)>>> = RefCell::new(HashMap::new());
    /// the model of the agent in view (ui.rs sets it each frame)
    static MODEL: RefCell<String> = const { RefCell::new(String::new()) };
}

/// The size of a marker's image: the decoded copy beside its `.b64`
/// in the store (`<hash>.<ext>`), read once. None when it is gone.
#[allow(dead_code)] // the history helpers: render.rs (track F) calls them
fn marker_size(m: &bend_images::Marker) -> Option<(u32, u32)> {
    if let Some(s) = SIZES.with(|c| c.borrow().get(&m.b64).copied()) {
        return s;
    }
    let size = bend_images::Kind::from_mime(&m.mime).and_then(|k| {
        let file = Path::new(&m.b64).with_extension(k.ext());
        let bytes = std::fs::read(file).ok()?;
        bend_images::dimensions(&bytes)
    });
    SIZES.with(|c| c.borrow_mut().insert(m.b64.clone(), size));
    size
}

#[allow(dead_code)] // the history helpers: render.rs (track F) calls them
fn wxh((w, h): (u32, u32)) -> String {
    format!("{w}×{h}")
}

/// `text` as spans in `style`, each image marker an accent chip
/// `▣ login.png` (a user line of the history; one line, no `\n`).
#[allow(dead_code)] // the history helpers: render.rs (track F) calls them
pub(crate) fn chip_spans(text: &str, style: Style) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    let mut last = 0usize;
    for m in bend_images::markers(text) {
        let before = text.get(last..m.start).unwrap_or("");
        if !before.is_empty() {
            out.push(Span::styled(before.to_string(), style));
        }
        out.push(Span::styled(format!("{G_IMAGE} {}", short_name(&m.path)), chip_style()));
        last = m.end;
    }
    let rest = text.get(last..).unwrap_or("");
    if !rest.is_empty() || out.is_empty() {
        out.push(Span::styled(rest.to_string(), style));
    }
    out
}

/// The dim line under a user line with images: each image's size,
/// `▣ login-mobile.png 1170×2532 · ▣ clipboard 2048×1536`. None
/// without images.
#[allow(dead_code)] // the history helpers: render.rs (track F) calls them
pub(crate) fn sizes_line(text: &str) -> Option<String> {
    let parts: Vec<String> = bend_images::markers(text)
        .iter()
        .map(|m| match marker_size(m) {
            Some(s) => format!("{G_IMAGE} {} {}", short_name(&m.path), wxh(s)),
            None => format!("{G_IMAGE} {}", short_name(&m.path)),
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// A tool result holding images: `result · ▣ screenshot.png 390×844`
/// (dim, accent chip, dim size). None without images.
#[allow(dead_code)] // the history helpers: render.rs (track F) calls them
pub(crate) fn result_spans(text: &str) -> Option<Vec<Span<'static>>> {
    let ms = bend_images::markers(text);
    if ms.is_empty() {
        return None;
    }
    let d = Style::default().fg(dim());
    let mut out = vec![Span::styled("result ·".to_string(), d)];
    for (i, m) in ms.iter().enumerate() {
        if i > 0 {
            out.push(Span::styled(" ·".to_string(), d));
        }
        out.push(Span::raw(" "));
        out.push(Span::styled(format!("{G_IMAGE} {}", short_name(&m.path)), chip_style()));
        if let Some(s) = marker_size(m) {
            out.push(Span::styled(format!(" {}", wxh(s)), d));
        }
    }
    Some(out)
}

/// `text` without its image markers (the text around a result's images).
#[allow(dead_code)] // the history helpers: render.rs (track F) calls them
pub(crate) fn without_markers(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0usize;
    for m in bend_images::markers(text) {
        out.push_str(text.get(last..m.start).unwrap_or(""));
        last = m.end;
    }
    out.push_str(text.get(last..).unwrap_or(""));
    out
}

// ---- a model without vision (book §17) ----

/// The model of the agent in view, for the no-vision line (ui.rs).
pub(crate) fn set_model(model: &str) {
    MODEL.with(|m| {
        if *m.borrow() != model {
            *m.borrow_mut() = model.to_string();
        }
    });
}

/// The provider refused images: its error says image (or vision) and
/// that it is not supported. Anthropic, Mistral and OpenAI-style
/// providers each word it their way.
#[allow(dead_code)] // the history helpers: render.rs (track F) calls them
pub(crate) fn is_no_vision(err: &str) -> bool {
    let e = err.to_lowercase();
    let about = ["image", "vision", "multimodal", "multi-modal"].iter().any(|w| e.contains(w));
    let refused = [
        "not support",
        "unsupported",
        "only supported",
        "not supported",
        "does not accept",
        "doesn't support",
        "not allowed",
        "not enabled",
        "not available for this model",
        "invalid content type",
        "unknown variant `image",
        "cannot read",
    ]
    .iter()
    .any(|w| e.contains(w));
    // our own attach errors ("image too large", "not a PNG…") are not it
    about && refused && !e.contains("image too large") && !e.contains("not a png")
}

/// The no-vision line: `✗ {model} can't read images.` (error) then the
/// way out (dim). None when `err` is another error.
#[allow(dead_code)] // the history helpers: render.rs (track F) calls them
pub(crate) fn no_vision(err: &str) -> Option<Vec<Span<'static>>> {
    if !is_no_vision(err) {
        return None;
    }
    let model = MODEL.with(|m| m.borrow().clone());
    Some(no_vision_spans(if model.is_empty() { "this model" } else { &model }))
}

#[allow(dead_code)] // the history helpers: render.rs (track F) calls them
fn no_vision_spans(model: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(format!("  {G_FAILED} "), Style::default().fg(error())),
        Span::styled(format!("{model} can't read images."), Style::default().fg(error())),
        Span::styled(
            " pick a model that can (/model), or describe the screen in words.".to_string(),
            Style::default().fg(dim()),
        ),
    ]
}

// ---- the strip above the composer (book §14) ----

/// `310 kB`, `1.1 MB` (decimal units, like Finder).
pub(crate) fn size_text(bytes: u64) -> String {
    if bytes < 1_000 {
        format!("{bytes} B")
    } else if bytes < 999_500 {
        format!("{} kB", (bytes + 500) / 1_000)
    } else {
        let tenths = (bytes + 50_000) / 100_000;
        format!("{}.{} MB", tenths / 10, tenths % 10)
    }
}

/// One strip row: `▣ 1 shots/login-mobile.png` and, dim on the right,
/// `1170×2532 · 310 kB` (` → resized to fit 2048` after a downscale).
pub(crate) fn strip_row(n: usize, info: &Info) -> (String, String) {
    let left = format!("{G_IMAGE} {n} {}", info.source);
    let mut right = format!("{} · {}", wxh((info.width, info.height)), size_text(info.bytes));
    if info.resized {
        right.push_str(&format!(" → resized to fit {}", bend_images::MAX_DIMENSION));
    }
    (left, right)
}

/// The strip header (book §17).
pub(crate) const STRIP_TITLE: &str = "attached · backspace on a chip removes it";

/// The key hints while images are attached (tui-screens.html).
pub(crate) const STRIP_HINT: &str = "ctrl+v paste image · @ file";

/// The attachments still in the composer text, by number.
fn shown(app: &App) -> Vec<(usize, &Attachment)> {
    let mut v: Vec<(usize, &Attachment)> = chips(&app.ed.text)
        .into_iter()
        .filter_map(|(_, _, n)| app.attachments.iter().find(|a| a.label == label(n)).map(|a| (n, a)))
        .collect();
    v.sort_by_key(|(n, _)| *n);
    v.dedup_by_key(|(n, _)| *n);
    v
}

/// How many rows the strip takes (0: no image attached).
pub(crate) fn strip_height(app: &App) -> u16 {
    match shown(app).len() {
        0 => 0,
        n => n as u16 + 1,
    }
}

/// The strip rows at `width` columns: the dim title, then one row per
/// image, its chip in accent, its size flush right (dim). A long path
/// keeps its end (the file name).
pub(crate) fn strip_lines(app: &App, width: usize) -> Vec<Line<'static>> {
    use unicode_width::UnicodeWidthStr;
    let rows = shown(app);
    if rows.is_empty() {
        return Vec::new();
    }
    let d = Style::default().fg(dim());
    let mut out = vec![Line::from(Span::styled(STRIP_TITLE.to_string(), d))];
    for (n, a) in rows {
        let (_, right) = strip_row(n, &a.info);
        let chip = format!("{G_IMAGE} {n}");
        let room = width.saturating_sub(chip.width() + 1 + right.width() + 2);
        let src = crate::ui::truncate_left(&a.info.source, room);
        let pad = width.saturating_sub(chip.width() + 1 + src.width() + right.width()).max(2);
        out.push(Line::from(vec![
            Span::styled(chip, chip_style()),
            Span::styled(format!(" {src}"), Style::default().fg(text())),
            Span::styled(" ".repeat(pad), d),
            Span::styled(right, d),
        ]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn att(n: usize, m: &str) -> Attachment {
        Attachment { label: label(n), marker: m.into(), info: Info::default() }
    }

    #[test]
    fn numbers_reuse_freed_labels() {
        let mut atts = vec![att(1, "m1"), att(2, "m2")];
        // [Image #1] was deleted from the text: it is forgotten, 1 is free
        assert_eq!(next_number(&mut atts, "hello [Image #2]"), 1);
        assert_eq!(atts.len(), 1);
        assert_eq!(next_number(&mut atts, ""), 1);
        assert!(atts.is_empty());
    }

    #[test]
    fn chips_are_found_by_char_index() {
        // `é` is 2 bytes, 1 char: indices are chars
        let t = "é [Image #1] x [Image #12][Image #] [Image #3";
        assert_eq!(chips(t), vec![(2, 12, 1), (15, 26, 12)]);
        assert!(chips("").is_empty());
        assert_eq!(chip_around(t, 5), Some((2, 12)));
        assert_eq!(chip_around(t, 2), None);
        assert_eq!(chip_around(t, 12), None);
        // a range touching a chip takes it whole
        assert_eq!(chip_widen(t, 11, 12), (2, 12));
        assert_eq!(chip_widen(t, 0, 3), (0, 12));
        assert_eq!(chip_widen(t, 12, 13), (12, 13));
        assert_eq!(chip_text("[Image #12]"), "▣ 12");
    }

    #[test]
    fn strip_text_sizes_and_resize() {
        assert_eq!(size_text(999), "999 B");
        assert_eq!(size_text(310_400), "310 kB");
        assert_eq!(size_text(1_100_000), "1.1 MB");
        assert_eq!(size_text(1_149_999), "1.1 MB");
        let shot = Info { source: "shots/login-mobile.png".into(), width: 1170, height: 2532, bytes: 310_000, resized: false };
        assert_eq!(
            strip_row(1, &shot),
            ("▣ 1 shots/login-mobile.png".to_string(), "1170×2532 · 310 kB".to_string())
        );
        let clip = Info { source: "clipboard".into(), width: 2048, height: 1536, bytes: 1_100_000, resized: true };
        assert_eq!(
            strip_row(2, &clip),
            ("▣ 2 clipboard".to_string(), "2048×1536 · 1.1 MB → resized to fit 2048".to_string())
        );
        assert_eq!(STRIP_TITLE, "attached · backspace on a chip removes it");
    }

    fn line_text(l: &Line) -> String {
        l.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn strip_lists_the_chips_still_in_the_text() {
        let mut app = crate::sb::bench::test_app();
        app.attachments = vec![
            Attachment { info: Info { source: "clipboard".into(), width: 2048, height: 1536, bytes: 1_100_000, resized: true }, ..att(2, "m2") },
            Attachment { info: Info { source: "shots/a.png".into(), width: 10, height: 20, bytes: 300, resized: false }, ..att(1, "m1") },
            Attachment { info: Info { source: "gone.png".into(), ..Info::default() }, ..att(3, "m3") },
        ];
        app.ed.set("compare [Image #1] with [Image #2]", 0);
        assert_eq!(strip_height(&app), 3);
        let ls: Vec<String> = strip_lines(&app, 60).iter().map(line_text).collect();
        assert_eq!(ls[0], STRIP_TITLE);
        assert!(ls[1].starts_with("▣ 1 shots/a.png") && ls[1].ends_with("10×20 · 300 B"), "{ls:?}");
        assert!(ls[2].starts_with("▣ 2 clipboard") && ls[2].ends_with("→ resized to fit 2048"), "{ls:?}");
        // the size stays flush right; a narrow strip cuts the path's head
        assert_eq!(unicode_width::UnicodeWidthStr::width(ls[1].as_str()), 60);
        let narrow: Vec<String> = strip_lines(&app, 26).iter().map(line_text).collect();
        assert!(narrow[1].contains('…') && narrow[1].ends_with("10×20 · 300 B"), "{narrow:?}");
        app.ed.set("no images", 0);
        assert_eq!(strip_height(&app), 0);
        assert!(strip_lines(&app, 60).is_empty());
    }

    #[test]
    fn history_chips_sizes_and_results() {
        let dir = std::env::temp_dir().join(format!("bise-i-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // a PNG header is enough for its size
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend_from_slice(&390u32.to_be_bytes());
        png.extend_from_slice(&844u32.to_be_bytes());
        png.extend_from_slice(&[8, 6, 0, 0, 0]);
        std::fs::write(dir.join("abc.png"), &png).unwrap();
        let b64 = dir.join("abc.b64").to_string_lossy().to_string();
        let mk = |name: &str, path: &str| format!("<image name=\"{name}\" path=\"{path}\" mime=\"image/png\" b64=\"{b64}\">");
        let t = format!("compare {} with {}, off", mk("[Image #1]", "shots/login-mobile.png"), mk("[Image #2]", "/nowhere/x/y.png"));
        let spans = chip_spans(&t, Style::default());
        let s: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(s, "compare ▣ login-mobile.png with ▣ y.png, off");
        assert_eq!(spans[1].style, chip_style());
        assert_eq!(sizes_line(&t).as_deref(), Some("▣ login-mobile.png 390×844 · ▣ y.png 390×844"));
        assert_eq!(sizes_line("plain"), None);
        assert_eq!(chip_spans("plain", Style::default()).len(), 1);
        let r = result_spans(&format!("{}\nok", mk("[Image #1]", "/tmp/screenshot.png"))).unwrap();
        let r: String = r.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(r, "result · ▣ screenshot.png 390×844");
        assert_eq!(without_markers(&format!("a{}b", mk("n", "p"))), "ab");
        assert!(result_spans("no image").is_none());
        // a marker whose image left the store: no size, still a chip
        let gone = t.replace("abc.b64", "zzz.b64");
        assert_eq!(sizes_line(&gone).as_deref(), Some("▣ login-mobile.png · ▣ y.png"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_vision_matches_provider_errors_only() {
        for e in [
            "turn failed: the model provider answered 400: Image input is not supported for this model",
            "turn failed: 400 {\"message\":\"Invalid content type. image_url is only supported by certain models.\"}",
            "turn failed: model does not support vision",
            "turn failed: this model doesn't support images",
        ] {
            assert!(is_no_vision(e), "{e}");
        }
        for e in [
            "turn failed: rate limited",
            "image not attached: image too large (9000x9000, 1 bytes) and it could not be downscaled",
            "shots/a.txt: not a PNG, JPEG, GIF or WebP image",
            "unsupported tool call",
        ] {
            assert!(!is_no_vision(e), "{e}");
        }
        set_model("glm-5");
        let l: String = no_vision("turn failed: 400 image input not supported").unwrap().iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(l, "  ✗ glm-5 can't read images. pick a model that can (/model), or describe the screen in words.");
        assert!(no_vision("turn failed: timeout").is_none());
    }
}
