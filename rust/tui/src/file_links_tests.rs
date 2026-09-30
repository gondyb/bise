//! File links (file_links.rs, BISE-264): what is a path (existing vs
//! missing, `:line`, relative to the feed's folders), its url, the
//! editor's command, and a click in the feed.

use super::*;
use crate::file_links::{self, launch, Launch, Target};
use crate::links;
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::path::{Path, PathBuf};

/// A fresh folder with `files` (relative paths) in it.
fn tree(name: &str, files: &[&str]) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-fl-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    for f in files {
        let p = d.join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "x\n").unwrap();
    }
    // macOS: /var is /private/var; the paths compare as the TUI resolves them
    let d = d.canonicalize().unwrap();
    file_links::forget_stats();
    d
}

fn t(p: &Path, line: Option<u32>, col: Option<u32>) -> Target {
    Target { path: p.to_path_buf(), line, col }
}

/// The linked texts of `s` and their urls.
fn linked(s: &str) -> Vec<(String, String)> {
    let (sp, urls) = links::collect(|| inline_spans(s, Style::default().fg(theme::text())));
    sp.iter()
        .filter_map(|x| {
            let tag = links::tag_of(x.style.add_modifier);
            (tag > 0).then(|| (x.content.to_string(), urls[tag as usize - 1].clone()))
        })
        .collect()
}

#[test]
fn the_line_and_column_come_off_the_path() {
    use file_links::split_line;
    assert_eq!(split_line("a/b.rs"), ("a/b.rs", None, None));
    assert_eq!(split_line("a/b.rs:12"), ("a/b.rs", Some(12), None));
    assert_eq!(split_line("a/b.rs:12:3"), ("a/b.rs", Some(12), Some(3)));
    assert_eq!(split_line("a/b.md#L7"), ("a/b.md", Some(7), None));
    assert_eq!(split_line("a/b.md#L7-L9"), ("a/b.md", Some(7), None));
    assert_eq!(split_line("a/b.md#L7C2"), ("a/b.md", Some(7), Some(2)));
    assert_eq!(split_line("a/b.rs:x"), ("a/b.rs:x", None, None));
    assert_eq!(split_line("a/b.rs:0"), ("a/b.rs:0", None, None));
}

#[test]
fn only_an_existing_file_is_a_target_relative_to_the_feeds_folders() {
    let wt = tree("wt", &["src/lib.rs", "only-wt.md"]);
    let ws = tree("ws", &["src/lib.rs", "README.md", "docs/guide.md"]);
    file_links::set_dirs(vec![wt.clone(), ws.clone()]);
    // the worktree first, then the workspace
    assert_eq!(file_links::target("src/lib.rs:4"), Some(t(&wt.join("src/lib.rs"), Some(4), None)));
    assert_eq!(file_links::target("./src/lib.rs"), Some(t(&wt.join("./src/lib.rs"), None, None)));
    assert_eq!(file_links::target("README.md"), Some(t(&ws.join("README.md"), None, None)));
    assert_eq!(file_links::target("docs/guide.md:3:9"), Some(t(&ws.join("docs/guide.md"), Some(3), Some(9))));
    // absolute
    let abs = ws.join("docs/guide.md");
    assert_eq!(file_links::target(&format!("{}:2", abs.display())), Some(t(&abs, Some(2), None)));
    // missing, a folder, not a path
    assert_eq!(file_links::target("src/missing.rs"), None);
    assert_eq!(file_links::target("docs"), None);
    assert_eq!(file_links::target("docs/"), None);
    assert_eq!(file_links::target("e.g."), None);
    assert_eq!(file_links::target("1.2.3"), None);
    assert_eq!(file_links::target("https://x.dev/a.md"), None);
    // another feed: its own folders
    file_links::set_dirs(vec![ws.clone()]);
    assert_eq!(file_links::target("only-wt.md"), None);
    file_links::set_dirs(Vec::new());
}

#[test]
fn prose_code_spans_and_markdown_links_to_files_are_links() {
    let ws = tree("md", &["rust/tui/src/links.rs", "notes.md"]);
    file_links::set_dirs(vec![ws.clone()]);
    let url = |p: &str, l: Option<u32>| file_links::url_of(&t(&ws.join(p), l, None));
    let got = linked("see rust/tui/src/links.rs:42, then `notes.md` and [the notes](notes.md#L3).");
    assert_eq!(
        got,
        vec![
            ("rust/tui/src/links.rs:42".into(), url("rust/tui/src/links.rs", Some(42))),
            ("notes.md".into(), url("notes.md", None)),
            ("the notes".into(), url("notes.md", Some(3))),
        ]
    );
    // ./ and a sentence's end; a missing file, a word inside another, a url: plain
    let got = linked("(./notes.md). missing.md, xnotes.md, https://notes.md/x and `rust/nope.rs`");
    assert_eq!(
        got,
        vec![("./notes.md".into(), url("./notes.md", None)), ("https://notes.md/x".into(), "https://notes.md/x".into())]
    );
    // a markdown link to the web stays a web link; to a missing file, text
    let got = linked("[gone](nope.md) and [web](https://w.dev)");
    assert_eq!(got, vec![("web".into(), "https://w.dev".into())]);
    file_links::set_dirs(Vec::new());
}

#[test]
fn a_file_url_goes_there_and_back() {
    let tg = t(Path::new("/tmp/a b/c#d%e.rs"), Some(12), Some(3));
    let u = file_links::url_of(&tg);
    assert_eq!(u, "file:///tmp/a%20b/c%23d%25e.rs#L12C3");
    assert!(links::linkable(&u));
    assert_eq!(file_links::target_of_url(&u), Some(tg));
    assert_eq!(file_links::target_of_url("file://localhost/etc/hosts"), Some(t(Path::new("/etc/hosts"), None, None)));
    assert_eq!(file_links::target_of_url("https://x.dev/a"), None);
    // OSC 8 gets the file, not the line
    assert!(links::osc8_open("i", &u).ends_with(";file:///tmp/a%20b/c%23d%25e.rs\x1b\\"));
}

#[test]
fn the_editor_is_bise_editor_then_visual_then_editor() {
    use file_links::editor_choice;
    assert_eq!(editor_choice(Some("zed"), Some("code"), Some("vim")).as_deref(), Some("zed"));
    assert_eq!(editor_choice(None, Some("code -w"), Some("vim")).as_deref(), Some("code -w"));
    assert_eq!(editor_choice(Some(" "), Some(""), Some("vim")).as_deref(), Some("vim"));
    assert_eq!(editor_choice(None, None, None), None);
}

#[test]
fn each_editor_gets_the_line_its_own_way() {
    let f = Path::new("/w/a.rs");
    let at = t(f, Some(12), None);
    let at_col = t(f, Some(12), Some(3));
    let plain = t(f, None, None);
    let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(launch(Some("code"), &at), Launch::Gui(v(&["code", "-g", "/w/a.rs:12"])));
    assert_eq!(
        launch(Some("/usr/local/bin/cursor --wait"), &at_col),
        Launch::Gui(v(&["/usr/local/bin/cursor", "--wait", "-g", "/w/a.rs:12:3"]))
    );
    assert_eq!(launch(Some("windsurf"), &plain), Launch::Gui(v(&["windsurf", "/w/a.rs"])));
    assert_eq!(launch(Some("zed"), &at), Launch::Gui(v(&["zed", "/w/a.rs:12"])));
    assert_eq!(launch(Some("subl"), &at_col), Launch::Gui(v(&["subl", "/w/a.rs:12:3"])));
    assert_eq!(launch(Some("idea"), &at), Launch::Gui(v(&["idea", "--line", "12", "/w/a.rs"])));
    assert_eq!(
        launch(Some("webstorm"), &at_col),
        Launch::Gui(v(&["webstorm", "--line", "12", "--column", "3", "/w/a.rs"]))
    );
    assert_eq!(launch(Some("webstorm"), &plain), Launch::Gui(v(&["webstorm", "/w/a.rs"])));
    // terminal editors: the terminal panel
    assert_eq!(launch(Some("vim"), &at), Launch::Term(v(&["vim", "+12", "/w/a.rs"])));
    assert_eq!(launch(Some("nvim"), &plain), Launch::Term(v(&["nvim", "/w/a.rs"])));
    assert_eq!(launch(Some("hx"), &at_col), Launch::Term(v(&["hx", "/w/a.rs:12:3"])));
    assert_eq!(launch(Some("nano"), &at_col), Launch::Term(v(&["nano", "+12,3", "/w/a.rs"])));
    assert_eq!(launch(Some("emacs -nw"), &at), Launch::Term(v(&["emacs", "-nw", "+12", "/w/a.rs"])));
    assert_eq!(
        launch(Some("'/Applications/My Ed/ed' -x"), &at),
        Launch::Term(v(&["/Applications/My Ed/ed", "-x", "/w/a.rs"]))
    );
    // none set: the default app
    assert_eq!(launch(None, &at), Launch::Default(f.to_path_buf()));
    assert_eq!(launch(Some("  "), &at), Launch::Default(f.to_path_buf()));
}

/// A plain click (never a double click of the one before).
fn click_at(app: &mut App, x: u16, y: u16) {
    app.mouse = Default::default();
    for kind in [MouseEventKind::Down(MouseButton::Left), MouseEventKind::Up(MouseButton::Left)] {
        input::on_mouse(app, &MouseEvent { kind, column: x, row: y, modifiers: KeyModifiers::NONE }, 30);
    }
}

#[test]
fn a_click_on_a_path_opens_it_in_the_editor_at_its_line() {
    let ws = tree("click", &["src/main.rs"]);
    file_links::set_dirs(vec![ws.clone()]);
    file_links::EDITOR.with(|e| *e.borrow_mut() = Some("code".into()));
    file_links::LAUNCHED.with(|l| l.borrow_mut().clear());
    let mut app = sb::bench::test_app();
    // the feed's folders: the workspace (the draw sets them)
    sb::bench::set_workspace(&mut app, &ws.to_string_lossy());
    app.events.push(Ev::Assistant("the bug is in src/main.rs:7 now".into()));
    let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
    term.draw(|f| draw_frame(&mut app, f)).unwrap();
    let buf = term.backend().buffer().clone();
    let (x, y) = (0..buf.area.height)
        .find_map(|y| {
            let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect();
            row.find("src/main.rs").map(|i| (row[..i].chars().count() as u16, y))
        })
        .expect("the path is on screen");
    click_at(&mut app, x + 2, y);
    let file = ws.join("src/main.rs").to_string_lossy().into_owned();
    let launched = file_links::LAUNCHED.with(|l| l.borrow().clone());
    assert_eq!(launched, vec![Launch::Gui(vec!["code".into(), "-g".into(), format!("{}:7", file)])]);
    assert!(app.flash.as_ref().is_some_and(|(n, _)| n == "opening main.rs:7 in code"), "{:?}", app.flash);
    // a terminal editor: the panel
    file_links::EDITOR.with(|e| *e.borrow_mut() = Some("nvim".into()));
    click_at(&mut app, x + 2, y);
    let last = file_links::LAUNCHED.with(|l| l.borrow().last().cloned());
    assert_eq!(last, Some(Launch::Term(vec!["nvim".into(), "+7".into(), file])));
    // a click on the plain text opens nothing
    click_at(&mut app, x.saturating_sub(4), y);
    assert_eq!(file_links::LAUNCHED.with(|l| l.borrow().len()), 2);
    file_links::EDITOR.with(|e| *e.borrow_mut() = None);
    file_links::set_dirs(Vec::new());
}

#[test]
fn a_done_tool_row_links_its_paths() {
    let ws = tree("tool", &["src/app.ts"]);
    file_links::set_dirs(vec![ws.clone()]);
    let (sp, urls) = links::collect(|| file_links::plain_spans("error in src/app.ts:3:1: boom", Style::default()));
    assert_eq!(urls, vec![file_links::url_of(&t(&ws.join("src/app.ts"), Some(3), Some(1)))]);
    let texts: Vec<&str> = sp.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(texts, vec!["error in ", "src/app.ts:3:1", ": boom"]);
    file_links::set_dirs(Vec::new());
}
