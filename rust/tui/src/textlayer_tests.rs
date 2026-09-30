//! BISE-290: the text layer: links found and marked, the selection, the
//! copy, what a click does; then on the real screens (a card, a popup,
//! the help, a tool's box).

use super::*;
use crate::app::App;
use crate::links::LinkBackend;
use crate::run::draw_frame;
use crate::wire::Ev;
use crossterm::event::KeyModifiers;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Terminal;

fn ev(kind: MouseEventKind, x: u16, y: u16) -> MouseEvent {
    MouseEvent { kind, column: x, row: y, modifiers: KeyModifiers::NONE }
}
const DOWN: MouseEventKind = MouseEventKind::Down(MouseButton::Left);
const DRAG: MouseEventKind = MouseEventKind::Drag(MouseButton::Left);
const UP: MouseEventKind = MouseEventKind::Up(MouseButton::Left);

/// A frame of `rows` in the text rect `r` of a 60x8 screen, finished
/// (its `begin_frame` before the rows are made: their marked links).
fn frame(rows: &[Line<'static>], r: Rect, sel: &TextMouse) -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 8));
    crate::links::begin_frame();
    crate::pointer::begin_frame();
    ratatui::widgets::Widget::render(Paragraph::new(rows.to_vec()), r, &mut buf);
    text(r);
    finish(&mut buf, sel);
    buf
}

fn row(b: &Buffer, y: u16) -> String {
    (0..b.area.width).map(|x| b[(x, y)].symbol().to_string()).collect()
}

fn col(b: &Buffer, y: u16, needle: &str) -> u16 {
    let r = row(b, y);
    r[..r.find(needle).unwrap_or_else(|| panic!("no {needle:?} in {r:?}"))].chars().count() as u16
}

#[test]
fn a_full_url_and_a_file_path_are_links_a_bare_domain_is_not() {
    let dir = std::env::temp_dir().join(format!("bise-textlayer-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("notes.md"), "x").unwrap();
    crate::file_links::set_dirs(vec![dir.clone()]);
    let r = Rect::new(2, 1, 56, 3);
    let b = frame(&[Line::from("see https://example.com/docs. or mistral.ai, notes.md")], r, &TextMouse::default());
    let links = last_links();
    assert_eq!(links.len(), 2, "{links:?}");
    let x = col(&b, 1, "https://");
    assert_eq!(links[0], Link { y: 1, x0: x, x1: x + 24, url: "https://example.com/docs".into(), tail: false });
    assert!(links[1].url.starts_with("file://") && links[1].url.ends_with("/notes.md"), "{:?}", links[1]);
    // drawn as links: underlined, tagged, in the hit map (OSC 8, the hand)
    assert!(b[(x, 1)].modifier.contains(Modifier::UNDERLINED));
    assert_ne!(crate::links::tag_of(b[(x, 1)].modifier), 0);
    assert!(!b[(x + 24, 1)].modifier.contains(Modifier::UNDERLINED), "the trailing dot is no link");
    assert!(crate::links::frame_hits().iter().any(|h| h.url == "https://example.com/docs" && h.x0 == x));
    assert_eq!(crate::pointer::at(x + 3, 1), crate::pointer::Shape::Pointer);
    let d = col(&b, 1, "mistral");
    assert!(!b[(d, 1)].modifier.contains(Modifier::UNDERLINED), "a bare domain is text");
}

#[test]
fn bise_s_own_copy_marks_its_links() {
    let copy = "no key yet? [console.mistral.ai](https://console.mistral.ai/api-keys). not now: ctrl+x.";
    assert_eq!(copy_plain(copy), "no key yet? console.mistral.ai. not now: ctrl+x.");
    assert_eq!(copy_plain("a [b] c [d](not a url) e"), "a [b] c [d](not a url) e");
    let r = Rect::new(0, 0, 60, 2);
    begin_frame();
    let spans = copy_spans(copy, Style::default());
    let b = frame(&[Line::from(spans)], r, &TextMouse::default());
    assert_eq!(row(&b, 0).trim_end(), "no key yet? console.mistral.ai. not now: ctrl+x.");
    let x = col(&b, 0, "console");
    assert_eq!(last_links(), vec![Link { y: 0, x0: x, x1: x + 18, url: "https://console.mistral.ai/api-keys".into(), tail: true }]);
}

#[test]
fn on_the_accent_tint_the_underline_takes_the_text_color() {
    let st = Style::default().bg(crate::theme::accent()).fg(crate::theme::on_accent());
    let b = frame(&[Line::from(Span::styled("go https://a.example/x now", st))], Rect::new(0, 0, 60, 1), &TextMouse::default());
    let x = col(&b, 0, "https");
    assert_eq!(b[(x, 0)].underline_color, crate::theme::on_accent());
    let b = frame(&[Line::from("go https://a.example/x now")], Rect::new(0, 0, 60, 1), &TextMouse::default());
    assert_eq!(b[(x, 0)].underline_color, crate::theme::accent());
}

#[test]
fn the_feed_s_links_under_a_text_drawn_over_them_go() {
    crate::links::begin_frame();
    crate::links::push_hit(crate::links::Hit { y: 1, x0: 0, x1: 10, tag: 1, url: "https://under.example".into(), id: "f".into() });
    crate::links::push_hit(crate::links::Hit { y: 6, x0: 0, x1: 10, tag: 2, url: "https://beside.example".into(), id: "g".into() });
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 8));
    begin_frame();
    text(Rect::new(0, 0, 60, 3));
    finish(&mut buf, &TextMouse::default());
    let urls: Vec<String> = crate::links::frame_hits().into_iter().map(|h| h.url).collect();
    assert_eq!(urls, vec!["https://beside.example".to_string()]);
}

#[test]
fn a_drag_selects_inside_its_rect_and_copies_the_text() {
    let r = Rect::new(4, 1, 30, 4);
    let rows = [Line::from("first line of the card"), Line::from("and https://x.example/a b")];
    let mut m = TextMouse::default();
    let b = frame(&rows, r, &m);
    let t = Instant::now();
    let x = col(&b, 1, "line");
    assert_eq!(m.on(&ev(DOWN, x, 1), t), Out::Took);
    // past the rect's right: clamped to it, the blanks out
    assert_eq!(m.on(&ev(DRAG, 59, 2), t), Out::Took);
    assert!(m.held() && m.selecting());
    let shown = frame(&rows, r, &m);
    assert_eq!(shown[(x, 1)].bg, crate::theme::selection_bg());
    assert_ne!(shown[(x - 1, 1)].bg, crate::theme::selection_bg());
    assert_eq!(m.on(&ev(UP, 59, 2), t), Out::Copy("line of the card\nand https://x.example/a b".into()));
    assert!(!m.held() && m.selecting(), "the selection stays shown");
    // a press outside every text rect: not the layer's, the selection goes
    assert_eq!(m.on(&ev(DOWN, 50, 6), t), Out::Pass);
    assert!(!m.selecting());
}

#[test]
fn a_double_click_selects_the_word_a_triple_click_the_row() {
    let r = Rect::new(0, 0, 40, 2);
    let rows = [Line::from("  copy the https://x.example/abc url")];
    let mut m = TextMouse::default();
    let b = frame(&rows, r, &m);
    let x = col(&b, 0, "x.example");
    let t = Instant::now();
    m.on(&ev(DOWN, x, 0), t);
    assert_eq!(m.on(&ev(UP, x, 0), t), Out::Open("https://x.example/abc".into()), "the first click opens");
    m.on(&ev(DOWN, x, 0), t + Duration::from_millis(100));
    assert_eq!(m.on(&ev(UP, x, 0), t), Out::Copy("https://x.example/abc".into()));
    m.on(&ev(DOWN, x, 0), t + Duration::from_millis(200));
    assert_eq!(m.on(&ev(UP, x, 0), t), Out::Copy("copy the https://x.example/abc url".into()));
}

#[test]
fn a_plain_click_off_a_link_is_the_screen_s_own_at_the_release() {
    let r = Rect::new(0, 0, 40, 2);
    let mut m = TextMouse::default();
    frame(&[Line::from("a card row")], r, &m);
    let t = Instant::now();
    assert_eq!(m.on(&ev(DOWN, 3, 0), t), Out::Took);
    assert_eq!(m.on(&ev(UP, 3, 0), t), Out::Click(ev(DOWN, 3, 0)));
    // a scroll, a move: never the layer's
    assert_eq!(m.on(&ev(MouseEventKind::ScrollDown, 3, 0), t), Out::Pass);
    assert_eq!(m.on(&ev(MouseEventKind::Moved, 3, 0), t), Out::Pass);
}

#[test]
fn a_copied_link_whose_label_is_not_its_url_keeps_the_url() {
    let r = Rect::new(0, 0, 60, 1);
    let mut m = TextMouse::default();
    begin_frame();
    let spans = copy_spans("get one at [the console](https://console.example/keys) now", Style::default());
    let b = frame(&[Line::from(spans)], r, &m);
    let t = Instant::now();
    m.on(&ev(DOWN, 0, 0), t);
    m.on(&ev(DRAG, 40, 0), t);
    let _ = b;
    assert_eq!(m.on(&ev(UP, 40, 0), t), Out::Copy("get one at the console (https://console.example/keys) now".into()));
}

#[test]
fn a_tool_s_output_links_its_urls_and_files() {
    let dir = std::env::temp_dir().join(format!("bise-textlayer-out-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("notes.md"), "x").unwrap();
    crate::file_links::set_dirs(vec![dir.clone()]);
    let ((), urls) = crate::links::collect(|| {
        let s = spans("fetched https://x.example/y and wrote notes.md", Style::default());
        assert_eq!(s.len(), 4, "{s:?}");
        assert_eq!(s[1].content, "https://x.example/y");
        assert_ne!(crate::links::tag_of(s[1].style.add_modifier), 0);
    });
    assert_eq!(urls.len(), 2);
    assert!(urls[1].ends_with("/notes.md"));
}

// ---- on the real screens ----

struct Screen {
    app: App,
    term: Terminal<LinkBackend<Vec<u8>>>,
}

impl Screen {
    fn new(app: App) -> Screen {
        let area = Rect::new(0, 0, 120, 36);
        let term = Terminal::with_options(LinkBackend::new(Vec::<u8>::new()), ratatui::TerminalOptions { viewport: ratatui::Viewport::Fixed(area) }).unwrap();
        let mut s = Screen { app, term };
        s.frame();
        s
    }

    fn frame(&mut self) -> Buffer {
        let app = &mut self.app;
        let done = self.term.draw(|f| draw_frame(app, f)).unwrap();
        let b = done.buffer.clone();
        let _ = self.term.backend().take_output();
        b
    }

    fn mouse(&mut self, kind: MouseEventKind, x: u16, y: u16) {
        crate::input::on_mouse(&mut self.app, &ev(kind, x, y), 36);
        self.frame();
    }

    fn find(&mut self, text: &str) -> (u16, u16) {
        let b = self.frame();
        for y in 0..b.area.height {
            let r = row(&b, y);
            if let Some(i) = r.find(text) {
                return (r[..i].chars().count() as u16, y);
            }
        }
        panic!("{text} not on screen:\n{}", (0..b.area.height).map(|y| row(&b, y)).collect::<Vec<_>>().join("\n"))
    }

    fn note(&self) -> String {
        self.app.flash.as_ref().map(|f| f.0.clone()).unwrap_or_default()
    }

    /// A drag over `text` on the screen: what it copied.
    fn drag_over(&mut self, text: &str) -> String {
        let (x, y) = self.find(text);
        self.mouse(DOWN, x, y);
        self.mouse(DRAG, x + text.chars().count() as u16 - 1, y);
        self.mouse(UP, x + text.chars().count() as u16 - 1, y);
        self.app.text.clear();
        crate::clipboard::test_clipboard().unwrap_or_default()
    }

    fn click(&mut self, text: &str) {
        let (x, y) = self.find(text);
        // far from the last press: never a double click
        self.app.mouse = Default::default();
        self.app.text = Default::default();
        self.mouse(DOWN, x + 1, y);
        self.mouse(UP, x + 1, y);
    }
}

fn opened() -> Option<String> {
    crate::links::OPENED.with(|o| o.borrow().last().cloned())
}


#[test]
fn a_card_selects_copies_and_opens_its_links_a_click_still_opens_it() {
    let mut app = crate::sb::bench::test_app_drained();
    crate::sb::set_cards_for_tests(&mut app, &[(12, "question", "perf", "the bundle is 4 MB, see https://perf.example/report. split it?\n1. yes\n2. no")]);
    let mut s = Screen::new(app);
    // the strip row: a drag copies its words, the card stays closed
    assert_eq!(s.drag_over("the bundle is 4 MB"), "the bundle is 4 MB");
    assert_eq!(s.note(), "copied 18 chars");
    assert!(!crate::sb::card_open_for_tests(&s.app));
    // a click on its url opens it, not the card
    s.click("https://perf.example");
    assert_eq!(opened().as_deref(), Some("https://perf.example/report"));
    assert_eq!(s.note(), "opening https://perf.example/report");
    assert!(!crate::sb::card_open_for_tests(&s.app));
    // a click elsewhere on the row does what it did: opens the card
    s.click("the bundle");
    s.click("the bundle");
    assert!(crate::sb::card_open_for_tests(&s.app), "the click on the row still opens the card");
    // the card view: its text copies, its url opens
    assert_eq!(s.drag_over("split it?"), "split it?");
    crate::links::OPENED.with(|o| o.borrow_mut().clear());
    s.click("perf.example/report");
    assert_eq!(opened().as_deref(), Some("https://perf.example/report"));
    // a pick still picks (the option row, at the release)
    assert!(crate::sb::card_open_for_tests(&s.app));
}

#[test]
fn the_popup_and_the_help_select_and_copy() {
    let mut app = crate::sb::bench::test_app_drained();
    app.ed.insert("/he");
    let mut s = Screen::new(app);
    assert_eq!(s.drag_over("/help"), "/help");
    assert_eq!(s.app.ed.text, "/he", "a drag picks nothing");
    s.app.ed.clear();
    s.app.help = Some(crate::help::Overlay::new(crate::help::Page::Shortcuts));
    s.frame();
    // a row of the page, not the tabs in its border
    assert_eq!(s.drag_over("preview the selected agent"), "preview the selected agent");
    assert_eq!(s.note(), "copied 26 chars");
    assert!(s.app.help.is_some(), "the help stays open");
}

#[test]
fn a_tool_box_opened_with_ctrl_o_links_its_output() {
    let mut app = crate::sb::bench::test_app_drained();
    let mut td = crate::wire::ToolData::bare(1, crate::wire::ToolState::Ok);
    td.name = Some("bash".into());
    td.args = Some(r#"{"command":"curl -sI https://box.example/x"}"#.into());
    td.result = Some((true, "HTTP/2 200\nlocation: https://box.example/y\n".into()));
    app.events.push(Ev::Tool(td));
    crate::input::toggle_everything(&mut app);
    let mut s = Screen::new(app);
    assert_eq!(s.drag_over("location:"), "location:");
    crate::links::OPENED.with(|o| o.borrow_mut().clear());
    s.click("box.example/y");
    assert_eq!(opened().as_deref(), Some("https://box.example/y"));
}

