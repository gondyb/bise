//! The worktree boxes in the panel (pr-design §4.1): the 6 rules at the
//! panel's widths (24, 31, 44), ctrl up and held, NO_COLOR, ASCII.

use super::places::{Place, Pr};
use super::*;
use crate::ctrlhint::{Held, Hold};
use ratatui::{backend::TestBackend, Terminal};

fn agent(name: &str, status: &str, place: &str) -> Agent {
    Agent { name: name.into(), status: status.into(), place_id: place.into(), ..Agent::default() }
}

fn pr(number: u64, review: &str, checks: &str) -> Pr {
    Pr {
        number,
        url: format!("https://github.com/acme/web/pull/{number}"),
        state: "open".into(),
        review: review.into(),
        checks: checks.into(),
        ..Pr::default()
    }
}

fn place(dir: &str, agents: &[&str], pr: Option<Pr>, lid: Option<&str>) -> Place {
    Place {
        id: format!("wt:{dir}"),
        branch: Some(format!("sb/{dir}")),
        agents: agents.iter().map(|a| a.to_string()).collect(),
        pr,
        lid: lid.map(Into::into),
    }
}

/// The mock's panel (pr-support.html): main and sad-404 in your folder,
/// dark-mode and i18n sharing a worktree with PR #412 (changes asked),
/// login-fix's #415 failing, emoji-csv with no PR yet, one inbox item.
/// Numbers: dark-mode 1, sad-404 2, login-fix 3, i18n 4, emoji-csv 5
/// (creation order), so the boxes reorder the rows, not the numbers.
fn mock() -> App {
    let mut app = bench::test_app_drained();
    let sb = &mut app.sb;
    sb.agents = vec![
        Agent { main: true, turn_ms: Some(60_000), ..agent("main", "working", "shared") },
        Agent { turn_ms: Some(180_000), ..agent("dark-mode", "working", "wt:dark-mode") },
        agent("sad-404", "idle", "shared"),
        Agent { turn_ms: Some(300_000), ..agent("login-fix", "working", "wt:login-fix") },
        Agent { turn_ms: Some(42_000), ..agent("i18n", "working", "wt:dark-mode") },
        agent("emoji-csv", "waiting", "wt:emoji-csv"),
    ];
    let failing = Pr { failing: vec!["e2e/login".into()], ..pr(415, "none", "fail") };
    sb.places = vec![
        place("dark-mode", &["dark-mode", "i18n"], Some(pr(412, "changes_requested", "pass")), None),
        place("login-fix", &["login-fix"], Some(failing), None),
        place("emoji-csv", &["emoji-csv"], None, Some("no PR yet · 2 commits")),
    ];
    sb.flow = "pr".into();
    sb.activity.insert("i18n".into());
    sb.cards.push(cards::Card {
        id: 1,
        kind: "question".into(),
        agent: "sad-404".into(),
        text: "#409 is approved, checks pass. merge it?".into(),
        age_ms: 0,
        seen_at: std::time::Instant::now(),
        note: String::new(),
        look: None,
    });
    app
}

fn hold(app: &mut App) {
    app.hold = Hold::of(Held::Ctrl, std::time::Instant::now() - std::time::Duration::from_secs(2));
}

/// The panel `w` wide drawn at column 1 (column 0 is the blank column
/// left of it, where the rails go), `h` rows; the rows trimmed.
fn rows(app: &App, w: u16, h: u16) -> Vec<String> {
    buffer(app, w, h).1
}

fn buffer(app: &App, w: u16, h: u16) -> (ratatui::buffer::Buffer, Vec<String>) {
    let mut term = Terminal::new(TestBackend::new(w + 1, h)).unwrap();
    term.draw(|f| panel::draw_panel(app, f, Rect::new(1, 0, w, h))).unwrap();
    let buf = term.backend().buffer().clone();
    let rows = buf
        .content
        .chunks((w + 1) as usize)
        .map(|r| r.iter().map(|c| c.symbol()).collect::<String>().trim_end().to_string())
        .collect();
    (buf, rows)
}

fn show(rows: &[String]) -> String {
    rows.join("\n")
}

/// Rule 1 (order), 4 (every worktree a box, ψ out of the rows), the
/// rail and the closing `╰`, at 31 columns, at rest: the mock's frame.
#[test]
fn boxes_at_31_columns() {
    let app = mock();
    let r = rows(&app, panel_w(150), 24);
    let want = [
        "  agents",
        "",
        "  0 ∿ main :*       1m",
        "  2 ? sad-404",
        "",
        "╭─ ψ sb/dark-mode ───────── ↑ ─",
        "│ 1 ∿ dark-mode     3m",
        "╰ 4 ∿ i18n •       42s",
        "",
        "╭─ ψ sb/login-fix ───────── ↑ ─",
        "╰ 3 ∿ login-fix     5m",
        "",
        "╭─ ψ sb/emoji-csv ─────────────",
        "╰ 5 … emoji-csv",
        "",
        "  inbox",
    ];
    for (k, line) in want.iter().enumerate() {
        assert_eq!(r[k], *line, "row {k}:\n{}", show(&r));
    }
    assert!(r[16].starts_with("  1 ? sad-404"), "{}", show(&r));
    // the columns line up with your folder's rows (ψ's column blank)
    let col = |l: &str, w: &str| l.char_indices().position(|(i, _)| l[i..].starts_with(w));
    assert_eq!(col(&r[2], "1m"), col(&r[6], "3m"), "{}", show(&r));
    // a long name takes ψ's column back before it is cut
    let mut long = mock();
    long.sb.agents[4].name = "i18n-everywhere".into();
    long.sb.places[0].agents[1] = "i18n-everywhere".into();
    let r = rows(&long, panel_w(150), 24);
    assert!(r.iter().any(|l| l.starts_with("╰ 4 ∿ i18n-everywhere 42s")), "{}", show(&r));
    // the selection follows the rows' order, the numbers stay
    let names: Vec<&str> = app.sb.nav().iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["main", "sad-404", "dark-mode", "i18n", "login-fix", "emoji-csv"]);
}

/// Rule 2: the lines in the rule color, ψ and the branch dim, ↑ dim when
/// open, red when checks fail; never the accent.
#[test]
fn the_border_colors() {
    let app = mock();
    let (buf, r) = buffer(&app, panel_w(150), 24);
    let y = |s: &str| r.iter().position(|l| l.contains(s)).unwrap() as u16;
    let at = |x: u16, y: u16| buf[(x, y)].clone();
    let dark = y("sb/dark-mode");
    assert_eq!(at(0, dark).symbol(), "╭");
    assert_eq!(at(0, dark).fg, rule());
    assert_eq!(at(3, dark).symbol(), "ψ");
    assert_eq!(at(3, dark).fg, dim());
    let arrow = |row: u16| (0..31).find(|x| at(*x, row).symbol() == "↑").map(|x| at(x, row).fg);
    assert_eq!(arrow(dark), Some(dim()));
    assert_eq!(arrow(y("sb/login-fix")), Some(error()));
    assert_eq!(at(0, y("i18n")).fg, rule());
    for row in 0..24 {
        for x in 0..31 {
            if at(x, row).symbol() == "↑" {
                assert_ne!(at(x, row).fg, accent());
            }
        }
    }
}

/// Rule 5: ctrl held, the border adds the number, the lid line says the
/// state at the border's text column, before any agent; the rows get
/// their state words. The header adds the PRs and the flow.
#[test]
fn ctrl_held_opens_the_lid() {
    let mut app = mock();
    hold(&mut app);
    let r = rows(&app, panel_w(150), 26);
    let at = |s: &str| r.iter().position(|l| l.contains(s)).unwrap_or_else(|| panic!("{s}:\n{}", show(&r)));
    let dark = at("sb/dark-mode");
    assert_eq!(r[dark], "╭─ ψ sb/dark-mode ──── ↑ #412 ─", "{}", show(&r));
    assert_eq!(r[dark + 1], "│  changes asked · checks pass");
    assert!(r[dark + 2].starts_with("│ 1 ∿ dark-mode") && r[dark + 2].ends_with("working"), "{}", show(&r));
    let login = at("sb/login-fix");
    assert_eq!(r[login + 1], "│  checks fail: e2e/login");
    let csv = at("sb/emoji-csv");
    assert_eq!(r[csv], "╭─ ψ sb/emoji-csv ─────────────");
    assert_eq!(r[csv + 1], "│  no PR yet · 2 commits");
    assert!(r[csv + 2].starts_with("╰ 5 … emoji-csv"), "{}", show(&r));
    // the header: `↑ 2 PRs` with the counts, the flow after the folder
    app.sb.workspace = "/w/acme".into();
    let h: String = app.sb.summary(200, false, true, &[]).iter().map(|s| s.content.to_string()).collect();
    assert!(h.starts_with("/w/acme · lands via PRs · "), "{h}");
    assert!(h.contains(&format!("{} 2 PRs · # 1 in the inbox", G_PR)), "{h}");
    // at rest: nothing new in the header
    let rest = mock();
    let h: String = rest.sb.summary(200, false, false, &[]).iter().map(|s| s.content.to_string()).collect();
    assert!(!h.contains("PR") && !h.contains("lands"), "{h}");
    app.sb.flow = "trunk".into();
    let h: String = app.sb.summary(200, false, true, &[]).iter().map(|s| s.content.to_string()).collect();
    assert!(h.starts_with("/w/acme · lands on main · "), "{h}");
}

/// The panel's width on a `width`-column screen, framed.
fn panel_w(width: u16) -> u16 {
    crate::layout::cols(width, 40).panel.unwrap().w
}

/// Rule 3: the 24-column panel (90-99 wide): the branch is cut with `…`
/// first, ψ and ↑ stay, a border never wraps; the rows keep their
/// columns. The widest panel (44): the whole branch.
#[test]
fn boxes_at_24_and_44_columns() {
    let mut app = mock();
    app.sb.places[0].branch = Some("sb/dark-mode-everywhere".into());
    let narrow = panel_w(95);
    let r = rows(&app, narrow, 24);
    let dark = r.iter().find(|l| l.contains("sb/dark")).unwrap();
    assert_eq!(dark, "╭─ ψ sb/dark-mode-ev… ─ ↑ ─", "{}", show(&r));
    assert!(r.iter().all(|l| l.chars().count() <= narrow as usize + 1), "{}", show(&r));
    hold(&mut app);
    let r = rows(&app, narrow, 26);
    let dark = r.iter().position(|l| l.contains("sb/dark")).unwrap();
    assert_eq!(r[dark], "╭─ ψ sb/dark-mo… ─ ↑ #412 ─", "{}", show(&r));
    assert_eq!(r[dark + 1], "│  changes asked · checks…", "{}", show(&r));
    let wide = panel_w(400);
    let r = rows(&app, wide, 26);
    let dark = r.iter().position(|l| l.contains("sb/dark")).unwrap();
    assert_eq!(r[dark].chars().count(), wide as usize + 1);
    assert!(r[dark].starts_with("╭─ ψ sb/dark-mode-everywhere ──") && r[dark].ends_with("─ ↑ #412 ─"), "{}", show(&r));
}

/// Rule 6: a box never splits across the scroll: it goes under `+ n
/// more` whole (counted by its agents); a selected agent in a box
/// brings its whole box into view.
#[test]
fn a_box_never_splits() {
    let mut app = mock();
    // 2 title rows + 10 body rows: main, sad-404, blank, dark-mode's box
    // (3 rows), blank, login-fix's box (2 rows), then the cut
    let r = rows(&app, panel_w(150), 12);
    assert!(r.iter().any(|l| l.contains("sb/login-fix")) || r.iter().any(|l| l.contains("+ ")), "{}", show(&r));
    for name in ["sb/dark-mode", "sb/login-fix", "sb/emoji-csv"] {
        if let Some(y) = r.iter().position(|l| l.contains(name)) {
            // its last row is on screen
            let close = r[y..].iter().position(|l| l.starts_with('╰'));
            assert!(close.is_some(), "{name} split:\n{}", show(&r));
        }
    }
    let more = r.iter().find(|l| l.contains("+ ")).unwrap_or_else(|| panic!("{}", show(&r)));
    assert!(more.contains("more"), "{more}");
    // the selection on emoji-csv: its box whole on screen
    app.sb.selected = Some(5);
    let r = rows(&app, panel_w(150), 12);
    let y = r.iter().position(|l| l.contains("sb/emoji-csv")).unwrap_or_else(|| panic!("{}", show(&r)));
    assert!(r[y + 1].starts_with("╰ 5"), "{}", show(&r));
    for name in ["sb/dark-mode", "sb/login-fix"] {
        if let Some(y) = r.iter().position(|l| l.contains(name)) {
            assert!(r[y..].iter().any(|l| l.starts_with('╰')), "{name} split:\n{}", show(&r));
        }
    }
    // no box row ever shows without its border above it
    for h in 6..24 {
        for sel in [None, Some(0), Some(3), Some(5)] {
            app.sb.selected = sel;
            let r = rows(&app, panel_w(150), h);
            let mut open = false;
            for l in &r[2..] {
                if l.starts_with('╭') {
                    open = true;
                } else if l.starts_with('│') || l.starts_with('╰') {
                    assert!(open, "h {h} sel {sel:?}: a box without its border\n{}", show(&r));
                    if l.starts_with('╰') {
                        open = false;
                    }
                }
            }
        }
    }
}

/// `NO_COLOR`: the failing ↑ and the words `checks fail` are bold (no
/// red to say it); `BISE_ASCII=1`: ↑ is `P`, the box `+ - |`, never `^`.
#[test]
fn no_color_and_ascii() {
    let mut app = mock();
    hold(&mut app);
    let failing = app.sb.places[1].live_pr().unwrap().clone();
    let words = |p: &Pr| p.words(Style::default().fg(dim()));
    crate::theme::set_ascii_for_tests(true);
    let border: String = app.sb.places[0].border(31, true).spans.iter().map(|s| s.content.to_string()).collect();
    crate::theme::set_ascii_for_tests(false);
    assert!(border.contains("P #412") && !border.contains('^') && !border.contains('↑'), "{border}");
    // the legend row
    let row = crate::theme::LEGEND.iter().find(|s| s.glyph == G_PR).unwrap();
    assert_eq!(row.ascii, "P");
    assert!(row.meaning.contains("pull request"));
    // NO_COLOR: bold, not red
    let was = std::env::var_os("NO_COLOR");
    std::env::set_var("NO_COLOR", "1");
    let bold = failing.mark_style().add_modifier.contains(Modifier::BOLD);
    let w = words(&failing);
    match was {
        Some(v) => std::env::set_var("NO_COLOR", v),
        None => std::env::remove_var("NO_COLOR"),
    }
    assert!(bold);
    let fail = w.iter().find(|s| s.content == "checks fail").unwrap();
    assert!(fail.style.add_modifier.contains(Modifier::BOLD) && fail.style.fg != Some(error()));
}

/// The divider of an agent in a worktree (pr-design §4): `ψ branch with
/// i18n · ↑ #412`, the number a link; ctrl held, the PR's words after
/// the number.
#[test]
fn the_divider_says_the_branch_and_the_pr() {
    let mut app = mock();
    app.sb.focus = "dark-mode".into();
    let who = panel::viewed_who(&app);
    assert_eq!(who.place.as_deref(), Some("sb/dark-mode"));
    assert_eq!(who.with, ["i18n"]);
    let pr = who.pr.as_ref().unwrap();
    assert_eq!((pr.number, pr.url.as_str()), (412, "https://github.com/acme/web/pull/412"));
    assert!(pr.words.is_empty());
    hold(&mut app);
    let held = panel::viewed_who(&app);
    let words: String = held.pr.unwrap().words.iter().map(|s| s.content.to_string()).collect();
    assert_eq!(words, "changes asked · checks pass");
    // an agent alone in its worktree: no `with`; in your folder: nothing
    app.sb.focus = "login-fix".into();
    assert!(panel::viewed_who(&app).with.is_empty());
    app.sb.focus = "sad-404".into();
    let who = panel::viewed_who(&app);
    assert!(who.place.is_none() && who.pr.is_none());
}
