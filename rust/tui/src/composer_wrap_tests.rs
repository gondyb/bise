//! The composer across wrap boundaries: typed char by char, every frame
//! shows every text row, from the first, and the cursor. The height and
//! the draw once disagreed by one row when the text ended exactly at the
//! width (the cursor on the next row's end slot): the first row was
//! scrolled out for that one character.

use super::*;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use unicode_width::UnicodeWidthStr;
use voice::fakes::{FakeRecorder, FakeTranscriber};
use voice::Voice;

/// The screen row `y` from column `x`, `w` columns wide, as text: a wide
/// grapheme's hidden right cell is skipped; trailing blanks trimmed.
fn screen_row(buf: &ratatui::buffer::Buffer, x: u16, y: u16, w: usize) -> String {
    let mut s = String::new();
    let mut cx = x as usize;
    while cx < x as usize + w {
        let sym = buf[(cx as u16, y)].symbol();
        s.push_str(sym);
        cx += sym.width().max(1);
    }
    s.trim_end().to_string()
}

/// Draws `app` on a `width`×`height` screen and checks the composer:
/// no scroll, every drawn row on screen in order, the cursor reversed
/// at its row and column.
fn check_frame(app: &mut App, width: u16, height: u16, what: &str) {
    let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
    term.draw(|f| sb::draw_sb(app, f)).unwrap();
    let buf = term.backend().buffer().clone();
    let area = app.composer;
    let rows = editor::layout_input(&app.ed.text, area.w);
    let drawn = editor::drawn_rows(&rows, app.ed.cursor);
    assert_eq!(area.top, 0, "{what}: scrolled");
    assert!(area.h >= drawn, "{what}: {drawn} rows in a {}-row box", area.h);
    let (r, c) = editor::row_col(&rows, app.ed.cursor);
    // a pending accent: before the cursor when the row has a free
    // column, else in the cursor cell
    let accent = app.ed.pending_dead();
    let fits = rows[r].iter().map(|c| c.w).sum::<usize>() < area.w;
    for (i, row) in rows.iter().take(drawn).enumerate() {
        let mut want = String::new();
        for cell in row {
            let at_cursor = i == r && cell.ci == app.ed.cursor;
            match accent {
                Some(a) if at_cursor && fits => {
                    want.push(a);
                    want.push_str(cell.text);
                }
                Some(a) if at_cursor => want.push(a),
                _ if cell.newline => want.push(' '),
                _ => want.push_str(cell.text),
            }
        }
        let got = screen_row(&buf, area.x, area.y + i as u16, area.w);
        assert_eq!(got, want.trim_end(), "{what}: row {i}");
    }
    // the composer is the last of the screen: under its text, only its
    // blank bar row, the key bar and the frame's edge (book §8 "The frame")
    let lr = crate::layout::rows(width, height);
    assert!(area.h >= drawn.max(lr.min_text as usize), "{what}: {} rows for {drawn}", area.h);
    assert_eq!(area.y as usize + area.h + lr.pad_bottom as usize, lr.keybar as usize, "{what}: the key bar under the composer");
    let c = if accent.is_some() && fits { c + 1 } else { c };
    let cell = &buf[(area.x + c as u16, area.y + r as u16)];
    assert!(cell.modifier.contains(Modifier::REVERSED), "{what}: cursor at ({r}, {c})");
}

/// Types `text` grapheme by grapheme and checks every frame.
fn type_and_check(app: &mut App, text: &str, width: u16, height: u16) {
    use unicode_segmentation::UnicodeSegmentation;
    app.ed.clear();
    for (i, g) in text.graphemes(true).enumerate() {
        app.ed.insert(g);
        check_frame(app, width, height, &format!("{width}x{height} after {} graphemes", i + 1));
    }
}

/// A text 3½ rows long at `inner` columns, each row different.
fn letters(inner: usize) -> String {
    (0..inner * 3 + inner / 2).map(|i| (b'a' + (i % 26) as u8) as char).collect()
}

#[test]
fn typing_across_the_wrap_keeps_every_row_and_the_cursor() {
    for width in [30u16, 47, 80, 81, 120] {
        let mut app = sb::bench::test_app();
        let inner = width as usize - 6;
        type_and_check(&mut app, &letters(inner), width, 40);
        // words with spaces: the rows still break by width
        let words: String = "lorem ipsum dolor sit amet ".repeat(inner / 6);
        type_and_check(&mut app, &words, width, 40);
        // 2-column emojis: an odd width leaves a blank column at the end
        type_and_check(&mut app, &"👏".repeat(inner * 2), width, 40);
        type_and_check(&mut app, &"a👍🏽".repeat(inner), width, 40);
    }
}

#[test]
fn the_cursor_back_in_the_text_hides_the_end_slot_row() {
    let mut app = sb::bench::test_app();
    let text = letters(74);
    let exact: String = text.chars().take(74 * 2).collect();
    app.ed.set(&exact, 74 * 2);
    check_frame(&mut app, 80, 40, "cursor on the end slot");
    app.ed.set(&exact, 3);
    check_frame(&mut app, 80, 40, "cursor in the first row");
}

#[test]
fn recording_meter_narrows_the_rows_and_the_height_follows() {
    for width in [30u16, 80, 81] {
        let mut app = sb::bench::test_app();
        let (rec, tr) = (FakeRecorder::ok(true), FakeTranscriber::default());
        app.voice = Voice::new(true, Box::new(rec), Box::new(tr));
        assert!(voice_key(
            &mut app,
            &crossterm::event::KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
            || Some("sk-test".into())
        ));
        assert!(app.voice.active());
        let inner = width as usize - 8;
        let text = letters(inner);
        app.ed.clear();
        use unicode_segmentation::UnicodeSegmentation;
        for (i, g) in text.graphemes(true).enumerate() {
            app.ed.insert_voice(g);
            check_frame(&mut app, width, 40, &format!("recording {width} after {}", i + 1));
        }
    }
}

#[test]
fn a_pending_accent_never_pushes_the_cursor_off_the_row() {
    for width in [30u16, 80] {
        let inner = width as usize - 6;
        let mut app = sb::bench::test_app();
        let text = letters(inner);
        for n in [inner - 2, inner - 1, inner, inner + 1, 2 * inner - 1, 2 * inner] {
            let t: String = text.chars().take(n).collect();
            app.ed.set(&t, n);
            app.ed.dead_key('´');
            check_frame(&mut app, width, 40, &format!("accent after {n} at {width}"));
            // and in the middle of a full row
            app.ed.set(&t, n.saturating_sub(3));
            app.ed.dead_key('´');
            check_frame(&mut app, width, 40, &format!("accent mid-row {n} at {width}"));
        }
    }
}
