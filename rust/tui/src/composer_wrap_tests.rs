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
    // blank bar row (from 20 rows), the key bar and the frame's edge (book §8 "The frame")
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

/// One test per width (a fresh app each, as before): they run in parallel.
fn typing_across_the_wrap_at(width: u16) {
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

#[test]
fn typing_across_the_wrap_keeps_every_row_and_the_cursor_30() {
    typing_across_the_wrap_at(30);
}

#[test]
fn typing_across_the_wrap_keeps_every_row_and_the_cursor_47() {
    typing_across_the_wrap_at(47);
}

#[test]
fn typing_across_the_wrap_keeps_every_row_and_the_cursor_80() {
    typing_across_the_wrap_at(80);
}

#[test]
fn typing_across_the_wrap_keeps_every_row_and_the_cursor_81() {
    typing_across_the_wrap_at(81);
}

#[test]
fn typing_across_the_wrap_keeps_every_row_and_the_cursor_120() {
    typing_across_the_wrap_at(120);
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
fn recording_with_the_voice_chip_the_height_follows() {
    for width in [30u16, 80, 81] {
        let mut app = sb::bench::test_app();
        let (rec, tr) = (FakeRecorder::ok(true), FakeTranscriber::default());
        app.voice = Voice::new(true, Box::new(rec), Box::new(tr));
        assert!(voice_key(
            &mut app,
            &crossterm::event::KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
            || Ok(voice::fakes::job())
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
    // chips are words too (a chip `[Image #1]` is drawn ` ▣ 1 `, 5 columns)
    assert_eq!(row_texts("look [Image #1] here", 11), vec!["look [Image #1]", "here"]);
    assert_eq!(row_texts("look [Image #1] here", 10), vec!["look", "[Image #1] here", ""]);
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
    // BISE-108 and the user's feedback on it, then the attachments box
    // (the user's pick "d"): 1 blank tinted row under the divider, the
    // box (a dim rounded frame, its rows at x0 + 3, no bar), then the
    // body right under it: the bar at x0 on every row of it (a blank bar
    // row under the text from 20 rows, and above it when there is no box),
    // in accent with text or images, faint when empty; the text at x0 + 3
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
            let pad_top = if n > 0 { 0 } else { rows.pad_top };
            let (top, bottom) = (a.y - pad_top, a.y + a.h as u16 + rows.pad_bottom);
            assert_eq!(bottom, rows.keybar, "{what}");
            // a blank bar row under the text from 20 rows (BISE-219)
            assert_eq!(rows.pad_bottom, u16::from(height >= 20), "{what}");
            let want = if empty { crate::theme::faint() } else { crate::theme::accent() };
            for y in top..bottom {
                let c = &buf[(x0, y)];
                assert_eq!((c.symbol(), c.fg), ("│", want), "{what}: row {y}");
            }
            let end = cols.margin + cols.pane_w;
            let row = |y: u16| -> String { (x0..end).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>().trim_end().to_string() };
            // above the body: no bar (the box's edge is dim)
            for y in divider + 1..top {
                let c = &buf[(x0, y)];
                assert!(c.symbol() != "│" || c.fg == crate::theme::dim(), "{what}: row {y}");
            }
            if n > 0 {
                let first = divider + 1 + u16::from(height >= 20);
                if height >= 20 {
                    assert_eq!(row(divider + 1), "", "{what}: the blank row above the box");
                }
                assert!(row(first).starts_with("╭─ attached ─"), "{what}: {:?}", row(first));
                let img = row(first + 1);
                assert!(img.starts_with("│   ▣ 1  Screenshot 1.png") && !img.contains('/'), "{what}: {img:?}");
                assert!(img.ends_with("  │"), "{what}: {img:?}");
                // no blank row between the box and your message
                assert!(row(top - 1).starts_with('╰'), "{what}: {:?}", row(top - 1));
                let keys = row(rows.keybar);
                assert!(keys.contains("ctrl+v paste image"), "{what}: {keys:?}");
            }
            // the key bar keeps ⏎ send first
            assert!(row(rows.keybar).starts_with("⏎ send"), "{what}: {:?}", row(rows.keybar));
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

/// The 5 cells of the first chip on screen: from the cell before its
/// glyph (`▣`, `❝`, `"` in ASCII) to the cell 3 after it.
fn pill_cells(buf: &ratatui::buffer::Buffer, glyph: &str) -> Vec<ratatui::buffer::Cell> {
    let a = buf.area;
    for y in a.top()..a.bottom() {
        for x in a.left() + 1..a.right().saturating_sub(3) {
            if buf[(x, y)].symbol() == glyph && buf[(x + 1, y)].symbol() == " " {
                return (x - 1..x + 4).map(|cx| buf[(cx, y)].clone()).collect();
            }
        }
    }
    panic!("no chip {glyph} on screen");
}

fn composer_with(app: &mut App, text: &str, cursor: usize) {
    app.ed.set(text, cursor);
}

#[test]
fn a_chip_is_a_pink_pill_in_the_composer() {
    // BISE-205 (user: « on dirait trop du texte »): ` ❝ 1 ` on the pill
    // tint, glyph accent, number text, one padding cell each side, in
    // dark and light; the whole pill reversed under the cursor
    use ratatui::style::Modifier;
    for mode in [theme::Mode::Dark, theme::Mode::Light] {
        theme::set_mode(mode);
        let p = theme::palette_of(mode);
        let mut app = sb::bench::test_app();
        composer_with(&mut app, "[Quote #1] fix this [Image #2] ok", 33);
        let buf = draw(&mut app, 100, 30);
        for g in [theme::G_QUOTE, theme::G_IMAGE] {
            let cells = pill_cells(&buf, g);
            let s: String = cells.iter().map(|c| c.symbol()).collect();
            let n = if g == theme::G_QUOTE { "1" } else { "2" };
            assert_eq!(s, format!(" {g} {n} "), "{mode:?}");
            assert!(cells.iter().all(|c| c.bg == p.pill), "{mode:?} {g}: {cells:?}");
            assert_eq!(cells[1].fg, p.accent, "{mode:?} {g}");
            assert_eq!(cells[3].fg, p.text, "{mode:?} {g}");
            assert!(cells.iter().all(|c| !c.modifier.contains(Modifier::REVERSED)));
        }
        // the text right after a pill is back on the composer's tint
        let mut app = sb::bench::test_app();
        composer_with(&mut app, "[Quote #1] fix", 0);
        let buf = draw(&mut app, 100, 30);
        let cells = pill_cells(&buf, theme::G_QUOTE);
        assert!(cells.iter().all(|c| c.modifier.contains(Modifier::REVERSED)), "{mode:?}: the cursor takes the pill");
    }
    theme::set_mode(theme::Mode::Dark);
}

#[test]
fn a_selected_chip_takes_the_selection_tint_whole() {
    let mut app = sb::bench::test_app();
    composer_with(&mut app, "see [Quote #1] ok", 17);
    app.ed.anchor = Some(0);
    app.ed.cursor = 16;
    let buf = draw(&mut app, 100, 30);
    let cells = pill_cells(&buf, theme::G_QUOTE);
    assert!(cells.iter().all(|c| c.bg == theme::selection_bg()), "{cells:?}");
}

#[test]
fn without_a_tint_the_chip_is_bracketed_same_width() {
    // ASCII (and NO_COLOR, the same form): `[" 1]`, no pill tint, 5
    // columns like the pill, so nothing moves between the forms
    theme::set_ascii_for_tests(true);
    let mut app = sb::bench::test_app();
    composer_with(&mut app, "[Quote #1] fix", 14);
    let buf = draw(&mut app, 100, 30);
    theme::set_ascii_for_tests(false);
    let a = buf.area;
    let found = (a.top()..a.bottom()).find_map(|y| {
        let row = screen_row(&buf, 0, y, a.width as usize);
        row.find("[\" 1] fix").map(|i| (y, row[..i].width() as u16))
    });
    let (y, x) = found.expect("no bracketed chip on screen");
    for cx in x..x + 5 {
        assert_ne!(buf[(cx, y)].bg, theme::palette().pill, "col {cx}");
    }
    assert_eq!(attach::chip_text("[Quote #1]").width(), 5);
}


/// The pane's rows under the divider, as text from column 0 (trailing
/// blanks trimmed).
fn pane_rows(width: u16, height: u16, images: usize, text: &str) -> Vec<String> {
    let mut app = sb::bench::test_app();
    if images > 0 {
        with_images(&mut app, images, text);
    } else {
        app.ed.insert(text);
    }
    let buf = draw(&mut app, width, height);
    let divider = (0..height).rev().find(|&y| buf[(0, y)].symbol() == "├" || buf[(0, y)].symbol() == "─").unwrap();
    (divider..height).map(|y| screen_row(&buf, 0, y, 40)).collect()
}

#[test]
fn the_typing_area_has_a_blank_bar_row_above_and_under_its_text() {
    // BISE-219 (user request: room around what you type; designer's
    // rows): from 20 rows, divider · bar row · text · bar row · key bar ·
    // edge; with the box, its blank row and the box take the top one;
    // 16-19 rows, neither (one alone looks like a bug); the pads stay
    // while the text grows to its max and scrolls
    let bar = "│  │";
    let rest = pane_rows(120, 40, 0, "");
    assert_eq!(rest.len(), 6, "{rest:#?}");
    assert!(rest[0].starts_with("├─ you → main"), "{rest:#?}");
    assert_eq!(rest[1], bar);
    assert!(rest[2].starts_with("│  │    what's on your mind?"), "{rest:#?}");
    assert_eq!(rest[3], bar);
    assert!(rest[4].starts_with("│  ⏎ send"), "{rest:#?}");
    assert!(rest[5].starts_with("╰─"), "{rest:#?}");
    let boxed = pane_rows(120, 40, 1, "hi");
    assert_eq!(boxed.len(), 9, "{boxed:#?}");
    assert_eq!(boxed[1], "│");
    assert!(boxed[2].starts_with("│  ╭─ attached"), "{boxed:#?}");
    assert!(boxed[4].starts_with("│  ╰─"), "{boxed:#?}");
    assert!(boxed[5].starts_with("│  │   ▣ 1  hi"), "{boxed:#?}");
    assert_eq!(boxed[6], bar);
    assert!(boxed[7].starts_with("│  ⏎ send"), "{boxed:#?}");
    let small = pane_rows(120, 18, 0, "");
    assert_eq!(small.len(), 4, "{small:#?}");
    assert!(small[1].starts_with("│  │    what's on your mind?"), "{small:#?}");
    assert!(small[2].starts_with("│  ⏎ send"), "{small:#?}");
    let small_boxed = pane_rows(120, 18, 1, "hi");
    assert!(small_boxed[1].starts_with("│  ╭─ attached"), "{small_boxed:#?}");
    assert!(small_boxed[4].starts_with("│  │   ▣ 1  hi"), "{small_boxed:#?}");
    assert!(small_boxed[5].starts_with("│  ⏎ send"), "{small_boxed:#?}");
    // long text: 12 text rows at most, the pads still there
    let long = pane_rows(120, 40, 0, &"word ".repeat(400));
    assert_eq!(long.len(), 1 + 1 + 12 + 1 + 2, "{long:#?}");
    assert_eq!(long[1], bar);
    assert!(long[2].starts_with("│  │  word"), "{long:#?}");
    assert!(long[13].starts_with("│  │  word"), "{long:#?}");
    assert_eq!(long[14], bar);
    assert!(long[15].starts_with("│  ⏎ send"), "{long:#?}");
}
