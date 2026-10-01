//! The worktrees in the panel (pr-design §4.1, dev-flow §3.1): one box
//! per worktree, git in its border, its agents inside behind a rail.
//! Git lives in borders, agents live in rows.
//!
//! The hub sends `places` (the worktrees only, in their first agent's
//! order, the frozen contract of switchboard's `place.rs`) and each
//! agent's `place_id`; this module reads them and draws what is git's:
//! the border (`╭─ ψ sb/dark-mode ──── ↑ ─`), the held lid line
//! (`changes asked · checks pass`), the PR's mark and words for the
//! divider and the header.

use super::*;
use unicode_width::UnicodeWidthStr;

/// A worktree as the hub sends it (`PlaceView`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Place {
    pub(crate) id: String,
    pub(crate) branch: Option<String>,
    /// its agents, not archived, in the hub's order
    pub(crate) agents: Vec<String>,
    pub(crate) pr: Option<Pr>,
    /// the held line the hub writes (`waits to land · 2nd`, `no PR yet ·
    /// 2 commits`): it wins over the PR's words, shown as it is
    pub(crate) lid: Option<String>,
}

/// The PR of a worktree's branch (`PrView`): the enums as their JSON
/// words (`changes_requested`), the failing checks' names.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Pr {
    pub(crate) number: u64,
    pub(crate) url: String,
    /// `draft`, `open`, `merged`, `closed`
    pub(crate) state: String,
    /// `none`, `pending`, `approved`, `changes_requested`
    pub(crate) review: String,
    /// `none`, `running`, `pass`, `fail`
    pub(crate) checks: String,
    pub(crate) failing: Vec<String>,
    /// how old the forge's last answer is, when it is late
    pub(crate) stale_ms: Option<u64>,
}

/// The snapshot's `places` (absent from an older hub: none).
pub(super) fn parse(v: &Value) -> Vec<Place> {
    let s = |x: &Value, k: &str| x.get(k).and_then(|b| b.as_str()).map(String::from);
    let Some(all) = v.get("places").and_then(|p| p.as_array()) else {
        return Vec::new();
    };
    all.iter()
        .map(|p| Place {
            id: s(p, "id").unwrap_or_default(),
            branch: s(p, "branch"),
            agents: p
                .get("agents")
                .and_then(|a| a.as_array())
                .map(|a| a.iter().filter_map(|n| n.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            pr: p.get("pr").filter(|x| x.is_object()).map(|x| {
                let checks = x.get("checks");
                Pr {
                    number: x.get("number").and_then(|n| n.as_u64()).unwrap_or(0),
                    url: s(x, "url").unwrap_or_default(),
                    state: s(x, "state").unwrap_or_default(),
                    review: s(x, "review").unwrap_or_default(),
                    checks: checks.and_then(|c| s(c, "state")).unwrap_or_default(),
                    failing: checks
                        .and_then(|c| c.get("failing"))
                        .and_then(|f| f.as_array())
                        .map(|f| f.iter().filter_map(|n| n.as_str().map(String::from)).collect())
                        .unwrap_or_default(),
                    stale_ms: x.get("stale_ms").and_then(|n| n.as_u64()),
                }
            }),
            lid: s(p, "lid").filter(|l| !l.is_empty()),
        })
        .collect()
}

impl Pr {
    /// Open or draft: merged and closed draw nothing (a merged place goes
    /// away a tick later; a closed one keeps its box, not its mark).
    pub(crate) fn live(&self) -> bool {
        matches!(self.state.as_str(), "open" | "draft")
    }

    fn fails(&self) -> bool {
        self.checks == "fail"
    }

    /// `↑`'s look (pr-design §4): dim open, faint draft or late, red
    /// when checks fail (bold under `NO_COLOR`); never pink, never green.
    pub(crate) fn mark_style(&self) -> Style {
        if self.stale_ms.is_some() || self.state == "draft" {
            Style::default().fg(faint())
        } else if self.fails() && no_color() {
            Style::default().add_modifier(Modifier::BOLD)
        } else if self.fails() {
            Style::default().fg(error())
        } else {
            Style::default().fg(dim())
        }
    }

    /// The PR's state in words, ctrl held, `st` their look: `changes
    /// asked · checks pass`, `draft · checks running`, `checks fail:
    /// e2e/login` (red only on the words `checks fail`), `state from 12m
    /// ago` when the forge is late.
    pub(crate) fn words(&self, st: Style) -> Vec<Span<'static>> {
        let mut parts: Vec<Vec<Span<'static>>> = Vec::new();
        let w = |t: &str| vec![Span::styled(t.to_string(), st)];
        if self.state == "draft" {
            parts.push(w("draft"));
        }
        match self.review.as_str() {
            "changes_requested" => parts.push(w("changes asked")),
            "approved" => parts.push(w("approved")),
            "pending" => parts.push(w("in review")),
            _ => {}
        }
        match self.checks.as_str() {
            "pass" => parts.push(w("checks pass")),
            "running" => parts.push(w("checks running")),
            "fail" => {
                let red = if no_color() { st.add_modifier(Modifier::BOLD) } else { st.fg(error()) };
                let mut v = vec![Span::styled("checks fail", red)];
                if !self.failing.is_empty() {
                    v.push(Span::styled(format!(": {}", self.failing.join(", ")), st));
                }
                parts.push(v);
            }
            _ => {}
        }
        if parts.is_empty() {
            parts.push(w("open"));
        }
        if let Some(ms) = self.stale_ms {
            parts.push(w(&format!("state from {} ago", panel::short_age(ms))));
        }
        let mut out = Vec::new();
        for (k, p) in parts.into_iter().enumerate() {
            if k > 0 {
                out.push(Span::styled(" · ", st));
            }
            out.extend(p);
        }
        out
    }
}

fn no_color() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())
}

impl Place {
    /// The PR when it is open or a draft.
    pub(crate) fn live_pr(&self) -> Option<&Pr> {
        self.pr.as_ref().filter(|p| p.live())
    }

    /// Trunk flow: its land waits in the queue (the hub's lid says
    /// `waits to land · 2nd`): `…` in its border (dev-flow §7).
    fn waits_to_land(&self) -> bool {
        self.lid.as_deref().is_some_and(|l| l.starts_with("waits to land"))
    }

    /// The top border, `w` columns from the panel's gap column (the
    /// rail's): `╭─ ψ sb/dark-mode ───── ↑ ─`; ctrl held (`held`) `↑
    /// #412`; a land waiting `… ─`. Short on room the branch is cut with
    /// `…` first, ψ and the mark stay; it never wraps.
    pub(crate) fn border(&self, w: usize, held: bool) -> Line<'static> {
        let line = Style::default().fg(rule());
        let d = Style::default().fg(dim());
        let mut right: Vec<Span<'static>> = Vec::new();
        if let Some(pr) = self.live_pr() {
            right.push(Span::styled(theme::pr_glyph(), pr.mark_style()));
            // the number only where it fits with ψ (a 2-column fill)
            if held && w >= 3 + 2 + 2 + 3 + 1 + format!(" #{}", pr.number).width() + 2 {
                let num = if pr.state == "draft" || pr.stale_ms.is_some() { Style::default().fg(faint()) } else { d };
                right.push(Span::styled(format!(" #{}", pr.number), num));
            }
        } else if self.waits_to_land() {
            right.push(Span::styled(theme::glyph(G_WAITING), d));
        }
        let right_w: usize = right.iter().map(|s| s.content.width()).sum();
        // `╭─ ` `ψ ` the branch ` ` at least one `─`, then ` mark ─`
        let tail = if right.is_empty() { 0 } else { right_w + 3 };
        let fixed = 3 + theme::glyph(G_WORKTREE).width() + 1 + 1 + tail;
        let room = w.saturating_sub(fixed + 1);
        let branch = match &self.branch {
            Some(b) if room >= 2 => panel::fit(b, room),
            _ => String::new(),
        };
        let mut spans = vec![
            Span::styled("╭─ ", line),
            Span::styled(theme::glyph(G_WORKTREE).to_string(), d),
        ];
        if !branch.is_empty() {
            spans.push(Span::styled(format!(" {}", branch), d));
        }
        spans.push(Span::raw(" "));
        let used: usize = spans.iter().map(|s| s.content.width()).sum();
        let fill = w.saturating_sub(used + tail);
        spans.push(Span::styled("─".repeat(fill), line));
        if !right.is_empty() {
            spans.push(Span::raw(" "));
            spans.extend(right);
            spans.push(Span::styled(" ─", line));
        }
        Line::from(spans)
    }

    /// The lid line, ctrl held, `w` columns from the gap column: dim, no
    /// glyph, at the border's text column (after the rail and 2 blanks):
    /// the hub's lid, else the PR's words; None when nothing to say.
    pub(crate) fn lid_line(&self, w: usize) -> Option<Line<'static>> {
        let d = Style::default().fg(dim());
        let words = match (&self.lid, self.live_pr()) {
            (Some(l), _) => vec![Span::styled(l.clone(), d)],
            (None, Some(pr)) => pr.words(d),
            (None, None) => return None,
        };
        let mut spans = vec![Span::styled("│", Style::default().fg(rule())), Span::raw("  ")];
        spans.extend(fit_spans(words, w.saturating_sub(4)));
        Some(Line::from(spans))
    }
}

/// `spans` cut to `max` columns, `…` at the cut (in the cut span's look).
pub(crate) fn fit_spans(spans: Vec<Span<'static>>, max: usize) -> Vec<Span<'static>> {
    let total: usize = spans.iter().map(|s| s.content.width()).sum();
    if total <= max {
        return spans;
    }
    let mut out = Vec::new();
    let mut used = 0;
    for s in spans {
        let w = s.content.width();
        if used + w < max {
            used += w;
            out.push(s);
            continue;
        }
        let cut = panel::fit(&s.content, max - used);
        out.push(Span::styled(cut, s.style));
        break;
    }
    out
}

/// The header's held count: the worktrees with an open PR (`↑ 2 PRs`).
pub(crate) fn open_prs(places: &[Place]) -> usize {
    places.iter().filter(|p| p.live_pr().is_some()).count()
}

/// What the header says of the flow, held, after the folder (dev-flow
/// §7): `lands via PRs`, `lands on main`; "" when the hub has not said.
pub(crate) fn flow_words(flow: &str) -> &'static str {
    match flow {
        "pr" => "lands via PRs",
        "trunk" => "lands on main",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(l: &Line) -> String {
        l.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn pr(state: &str, review: &str, checks: &str) -> Pr {
        Pr { number: 412, url: "https://github.com/acme/web/pull/412".into(), state: state.into(), review: review.into(), checks: checks.into(), ..Pr::default() }
    }

    fn place(branch: &str, pr: Option<Pr>, lid: Option<&str>) -> Place {
        Place { id: "wt:x".into(), branch: Some(branch.into()), agents: vec!["x".into()], pr, lid: lid.map(Into::into) }
    }

    /// The contract's JSON (switchboard place.rs, `the_contract_json`)
    /// read back: the tagged checks, the snake_case words.
    #[test]
    fn reads_the_hubs_places() {
        let v = serde_json::json!({"places": [
            {"id": "wt:dark", "branch": "sb/dark", "agents": ["dark", "i18n"], "lid": null,
             "pr": {"number": 412, "url": "u", "state": "open", "review": "changes_requested",
                    "checks": {"state": "fail", "failing": ["ci/test"]}, "stale_ms": null}},
            {"id": "wt:csv", "branch": null, "agents": [], "pr": null, "lid": "waits to land · 2nd"}
        ]});
        let ps = parse(&v);
        assert_eq!(ps.len(), 2);
        assert_eq!(ps[0].agents, ["dark", "i18n"]);
        let p = ps[0].pr.as_ref().unwrap();
        assert_eq!((p.number, p.review.as_str(), p.checks.as_str()), (412, "changes_requested", "fail"));
        assert_eq!(p.failing, ["ci/test"]);
        assert_eq!(ps[1].pr, None);
        assert!(ps[1].waits_to_land());
        assert!(parse(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn the_border_carries_the_branch_and_the_mark() {
        let p = place("sb/dark-mode", Some(pr("open", "pending", "pass")), None);
        assert_eq!(text(&p.border(31, false)), "╭─ ψ sb/dark-mode ───────── ↑ ─");
        assert_eq!(text(&p.border(31, true)), "╭─ ψ sb/dark-mode ──── ↑ #412 ─");
        // 24 columns: the branch is cut first, ψ and ↑ stay
        let b = text(&p.border(24, true));
        assert_eq!(b, "╭─ ψ sb/dark… ─ ↑ #412 ─");
        assert_eq!(b.width(), 24);
        // no PR: no mark, the line runs to the end
        let none = place("sb/emoji-csv", None, None);
        assert_eq!(text(&none.border(31, false)), "╭─ ψ sb/emoji-csv ─────────────");
        // a merged or closed PR: no mark
        let closed = place("sb/x", Some(pr("closed", "none", "none")), None);
        assert!(!text(&closed.border(31, true)).contains('↑'));
        // trunk: a land waiting
        let land = place("sb/emoji-csv", None, Some("waits to land · 2nd"));
        assert_eq!(text(&land.border(31, false)), "╭─ ψ sb/emoji-csv ───────── … ─");
        // never wider than its room, whatever the branch
        for w in [12, 18, 24, 31, 44] {
            let long = place("sb/a-very-long-branch-name-for-sure", Some(pr("open", "none", "none")), None);
            assert_eq!(text(&long.border(w, true)).width(), w, "{w}");
        }
    }

    #[test]
    fn the_lid_says_the_prs_state() {
        let lid = |p: &Place| p.lid_line(31).map(|l| text(&l));
        assert_eq!(lid(&place("b", Some(pr("open", "changes_requested", "pass")), None)).unwrap(), "│  changes asked · checks pass");
        assert_eq!(lid(&place("b", Some(pr("draft", "none", "running")), None)).unwrap(), "│  draft · checks running");
        assert_eq!(lid(&place("b", Some(pr("open", "none", "none")), None)).unwrap(), "│  open");
        let failing = Pr { failing: vec!["e2e/login".into()], ..pr("open", "none", "fail") };
        assert_eq!(lid(&place("b", Some(failing.clone()), None)).unwrap(), "│  checks fail: e2e/login");
        let stale = Pr { stale_ms: Some(12 * 60_000), ..pr("open", "approved", "pass") };
        assert_eq!(lid(&place("b", Some(stale), None)).unwrap(), "│  approved · checks pass · s…");
        // the hub's lid wins, as it is
        assert_eq!(lid(&place("b", None, Some("no PR yet · 2 commits"))).unwrap(), "│  no PR yet · 2 commits");
        assert_eq!(lid(&place("b", Some(pr("open", "none", "pass")), Some("waits to land · 2nd"))).unwrap(), "│  waits to land · 2nd");
        assert_eq!(lid(&place("b", None, None)), None);
        // red only on the words `checks fail`
        let l = place("b", Some(failing), None).lid_line(31).unwrap();
        let red: Vec<&str> = l.spans.iter().filter(|s| s.style.fg == Some(error())).map(|s| s.content.as_ref()).collect();
        assert_eq!(red, ["checks fail"]);
    }
}
