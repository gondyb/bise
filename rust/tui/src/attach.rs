//! Image attachments in the composer (projects/switchboard/docs/images.md).
//!
//! The composer shows `[Image #N]`; the app keeps what each label
//! stands for. On send, every label still in the text becomes its
//! marker (`<image name=… b64=…>`, see the `bend-images` crate) and the
//! attachments start over. Sources: a picked `@` image, a paste that is
//! only image paths (a file dragged into the terminal), Ctrl+V (the
//! clipboard image). Nothing here panics on any input.

use crate::app::App;
use std::path::Path;

/// One attached image: its composer label and its marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Attachment {
    pub(crate) label: String,
    pub(crate) marker: String,
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

/// Put `path` in the image store and its label at the cursor.
pub(crate) fn attach_file(app: &mut App, path: &Path, source: &str) -> Result<String, String> {
    let stored = bend_images::store_file(path)?;
    Ok(add(app, source, &stored))
}

fn add(app: &mut App, source: &str, stored: &bend_images::Stored) -> String {
    let n = next_number(&mut app.attachments, &app.ed.text);
    let l = label(n);
    let marker = bend_images::marker(&l, source, stored);
    app.attachments.push(Attachment { label: l.clone(), marker });
    // a space before the label when it would touch a word
    let before = app.ed.text.chars().nth(app.ed.cursor.wrapping_sub(1));
    let pad = if app.ed.cursor > 0 && before.is_some_and(|c| !c.is_whitespace()) { " " } else { "" };
    app.ed.paste(&format!("{pad}{l} "));
    l
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
    let stored = bend_images::store_bytes(bytes)?;
    let source = stored.file.to_string_lossy().to_string();
    Ok(add(app, &source, &stored))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_reuse_freed_labels() {
        let mut atts = vec![
            Attachment { label: label(1), marker: "m1".into() },
            Attachment { label: label(2), marker: "m2".into() },
        ];
        // [Image #1] was deleted from the text: it is forgotten, 1 is free
        assert_eq!(next_number(&mut atts, "hello [Image #2]"), 1);
        assert_eq!(atts.len(), 1);
        assert_eq!(next_number(&mut atts, ""), 1);
        assert!(atts.is_empty());
    }
}
