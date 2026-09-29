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
    assert_eq!(area.y as usize + area.h + (lr.pad_bottom + lr.edge) as usize, lr.keybar as usize, "{what}: the key bar under the composer");
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

// ---- BISE-108: the composer with images and several rows ----

/// The composer's rows as text (the cells, newlines as a blank).
fn row_texts(text: &str, inner: usize) -> Vec<String> {
    editor::layout_input(text, inner)
        .iter()
        .map(|r| r.iter().map(|c| if c.newline { " " } else { c.text }).collect::<String>().trim_end().to_string())
        .collect()
}

#[test]
fn the_composer_wraps_at_word_boundaries() {
    // the user saw `le n` / `om complet`: a word moves whole to the next
    // row, the space stays at the end of its row
    let t = "je vois ça, le nom complet, ce qui fait trop";
    assert_eq!(row_texts(t, 16), vec!["je vois ça, le", "nom complet, ce", "qui fait trop"]);
    // every row fits, and no word is cut when it fits a row
    for inner in 12..40 {
        let text = "lorem ipsum dolor sit amet, consectetur adipiscing elit ".repeat(3);
        let rows = row_texts(&text, inner);
        let words: Vec<&str> = text.split_whitespace().collect();
        let got: Vec<&str> = rows.iter().flat_map(|r| r.split_whitespace()).collect();
        assert_eq!(got, words, "at {inner}: {rows:?}");
        for r in &rows {
            assert!(r.width() <= inner, "at {inner}: {r:?}");
        }
    }
    // a word longer than the row is cut (only then)
    assert_eq!(row_texts("ab abcdefghij", 6), vec!["ab", "abcdef", "ghij"]);
    // a space that doesn't fit takes the word before it along
    assert_eq!(row_texts("abc def ", 7), vec!["abc", "def"]);
    // chips are words too
    // chips are words too (a chip `[Image #1]` is drawn `▣ 1`, 3 columns)
    assert_eq!(row_texts("look [Image #1] here", 9), vec!["look [Image #1]", "here"]);
    assert_eq!(row_texts("look [Image #1] here", 8), vec!["look", "[Image #1] here", ""]);
}

/// A composer with `n` images from deep folders and `text` after them.
fn with_images(app: &mut App, n: usize, text: &str) {
    let deep = "/var/folders/c5/kw86k5nx0zg2xnbzpnrwsk8h0000gn/T/TemporaryItems/NSIRD_x";
    app.attachments = (1..=n)
        .map(|i| attach::Attachment {
            label: attach::label(i),
            marker: format!("m{i}"),
            info: attach::Info { source: format!("{deep}/Screenshot {i}.png"), width: 1788, height: 542, bytes: 83_000, resized: false },
        })
        .collect();
    let chips: String = (1..=n).map(|i| format!("{} ", attach::label(i))).collect();
    let t = format!("{chips}{text}");
    let len = t.chars().count();
    app.ed.set(&t, len);
}

fn draw(app: &mut App, width: u16, height: u16) -> ratatui::buffer::Buffer {
    let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
    term.draw(|f| sb::draw_sb(app, f)).unwrap();
    term.backend().buffer().clone()
}

#[test]
fn two_sections_the_attachments_then_the_body_behind_its_bar() {
    // BISE-108 and the user's feedback on it: the attachments (file
    // names), 1 blank tinted row, then the body: the bar at x0 on every
    // row of it (a blank bar row above and under the text by height), in
    // accent with text or images, faint when empty; the text at x0 + 3
    for (width, height) in [(200u16, 50u16), (120, 40), (120, 29), (80, 30), (80, 22), (80, 18)] {
        for (n, text, empty) in [(0, "", true), (0, "hello", false), (1, "one line", false), (2, &"word ".repeat(60)[..], false)] {
            let mut app = sb::bench::test_app();
            if n > 0 || !text.is_empty() {
                with_images(&mut app, n, text);
            }
            let buf = draw(&mut app, width, height);
            let cols = crate::layout::cols(width, height);
            let rows = crate::layout::rows(width, height);
            let x0 = cols.x0;
            let what = format!("{width}x{height} {n} images {text:?}");
            let divider = (0..height).rev().find(|&y| matches!(buf[(0, y)].symbol(), "├" | "─")).unwrap();
            let a = app.composer;
            assert_eq!(a.x, x0 + 3, "{what}");
            let (top, bottom) = (a.y - rows.pad_top, a.y + a.h as u16 + rows.pad_bottom);
            assert_eq!(bottom + rows.edge, rows.keybar, "{what}");
            let want = if empty { crate::theme::faint() } else { crate::theme::accent() };
            for y in top..bottom {
                let c = &buf[(x0, y)];
                assert_eq!((c.symbol(), c.fg), ("│", want), "{what}: row {y}");
            }
            let end = cols.margin + cols.pane_w;
            let row = |y: u16| -> String { (x0..end).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>().trim_end().to_string() };
            // above the body: no bar; the attachments at x0 + 3, then a blank row
            for y in divider + 1..top {
                assert_ne!(buf[(x0, y)].symbol(), "│", "{what}: row {y}");
            }
            if n > 0 {
                let first = divider + 1 + rows.edge;
                assert!(row(first).starts_with("   attached"), "{what}: {:?}", row(first));
                let img = row(first + 1);
                assert!(img.starts_with("   ▣ 1 Screenshot 1.png") && !img.contains('/'), "{what}: {img:?}");
                if height >= 20 {
                    assert_eq!(row(top - 1), "", "{what}: the blank row between the sections");
                }
                let keys = row(rows.keybar);
                assert!(keys.contains("ctrl+v paste image"), "{what}: {keys:?}");
            }
            // the key bar keeps ⏎ send first
            assert!(row(rows.keybar).starts_with("⏎ send"), "{what}: {:?}", row(rows.keybar));
            // tall screens: a plain tinted row under the divider and above the key bar
            if height >= 30 {
                assert_eq!(row(divider + 1), "", "{what}");
                assert_eq!(row(rows.keybar - 1), "", "{what}");
            }
        }
    }
}

#[test]
fn the_divider_state_never_runs_past_the_frame() {
    for width in [60u16, 80, 95, 100, 133, 160, 200, 220, 260] {
        let mut app = sb::bench::test_app();
        with_images(&mut app, 2, "four rows of text ");
        let height = 40;
        let buf = draw(&mut app, width, height);
        let divider = (0..height).rev().find(|&y| buf[(0, y)].symbol() == "├").unwrap();
        let row: String = (0..width).map(|x| buf[(x, divider)].symbol().to_string()).collect();
        // joined to the frame, the state ends at F − 4 then 1 space and 1 rule
        assert!(row.ends_with("─┤") || row.ends_with(" ─┤"), "{width}: {row}");
        let last_text = (0..width).rev().find(|&x| !matches!(buf[(x, divider)].symbol(), "─" | "┤" | " " | "┴")).unwrap();
        assert!(last_text <= width - 4, "{width}: {row}");
        // the frame's corners
        assert_eq!(buf[(0, 0)].symbol(), "╭", "{width}");
        assert_eq!(buf[(width - 1, 0)].symbol(), "╮", "{width}");
        assert_eq!(buf[(0, height - 1)].symbol(), "╰", "{width}");
        assert_eq!(buf[(width - 1, height - 1)].symbol(), "╯", "{width}");
        for y in 1..height - 1 {
            assert!(matches!(buf[(width - 1, y)].symbol(), "│" | "┤" | "┃"), "{width}: right side at {y}");
        }
    }
}
