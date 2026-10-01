//! Pull requests (pr-design §7-§10): the hub follows the PR of each
//! place's branch by polling the forge, never with a token of its own
//! (GitHub through `gh`, which has the user's login).
//!
//! - this module: the [`Forge`] seam, the repo of the workspace, and
//!   [`diff`], what changed between two answers (the hub's PR events);
//! - [`github`]: `gh api graphql`, one query per repo, an alias per branch
//!   (25 aliases measured at 1 point, like 2);
//! - [`poll`]: the cadence and back-off (§7) and the worker that asks,
//!   on its own thread (a query takes ~0.5 s: never on the hub's loop).
//!
//! The hub's side (`core.rs`, `Input::Prs`): the snapshots by place id
//! (`Hub::prs`), the journal lines that keep each place's PR number, the
//! held line of a place with no PR yet, and a merged PR's cleanup.
//! `activity` (reviews, comments) and `merge` join [`Forge`] with pr-news
//! and pr-merge (wave 3).

pub mod github;
pub mod poll;

use crate::place::{PrSnapshot, PrState};

/// A repository on a forge: `github.com/owner/name`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoRef {
    pub host: String,
    pub owner: String,
    pub name: String,
}

/// The repo of a remote's URL: `https://host/o/r(.git)`,
/// `git@host:o/r(.git)`, `ssh://git@host(:port)/o/r(.git)`.
pub fn repo_of_url(url: &str) -> Option<RepoRef> {
    let url = url.trim();
    let (host, path) = if let Some(rest) = url.split_once("://").map(|(_, r)| r) {
        let rest = rest.rsplit_once('@').map_or(rest, |(_, r)| r);
        let (host, path) = rest.split_once('/')?;
        (host.split(':').next()?.to_string(), path.to_string())
    } else {
        // scp-like: git@host:o/r
        let (user_host, path) = url.split_once(':')?;
        let host = user_host.rsplit_once('@').map_or(user_host, |(_, h)| h);
        (host.to_string(), path.to_string())
    };
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, name) = path.split_once('/')?;
    if host.is_empty() || owner.is_empty() || name.is_empty() || name.contains('/') {
        return None;
    }
    Some(RepoRef { host: host.to_lowercase(), owner: owner.into(), name: name.into() })
}

/// Why the forge did not answer (pr-design §7: the hub backs off, the
/// boxes keep their last state, faint; never an inbox item).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ForgeError {
    /// No `gh` on the hub's PATH.
    Missing,
    /// gh is not logged in, or its login is refused (401).
    Auth(String),
    RateLimited(String),
    /// No network, GitHub down.
    Offline(String),
    Other(String),
}

impl ForgeError {
    /// One line for hub.log and `bise doctor` (never gh's whole output).
    pub fn describe(&self) -> String {
        match self {
            ForgeError::Missing => "gh is not installed".into(),
            ForgeError::Auth(s) => format!("gh is not logged in ({})", s),
            ForgeError::RateLimited(s) => format!("GitHub's rate limit ({})", s),
            ForgeError::Offline(s) => format!("GitHub unreachable ({})", s),
            ForgeError::Other(s) => s.clone(),
        }
    }
}

/// One forge (GitHub now, GitLab later: pr-design §9).
pub trait Forge: Send {
    /// The latest PR whose head is each of `branches` (none for a branch
    /// without one): one call for them all.
    fn fetch(&self, repo: &RepoRef, branches: &[String]) -> Result<Vec<PrSnapshot>, ForgeError>;
}

/// The hub's PR events (pr-design §10). `Seen`, `Merged` and `Closed`
/// are journaled (the number survives a restart, so a restart never
/// sees a PR for the first time again); `Changed` and `Unreachable` go
/// to hub.log, and to pr-news (wave 3) for the agents and main.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrEvent {
    Seen { place: String, pr: PrSnapshot },
    Changed { place: String, from: PrSnapshot, to: PrSnapshot },
    Merged { place: String, pr: PrSnapshot },
    Closed { place: String, pr: PrSnapshot },
    Unreachable { error: ForgeError },
}

/// What the journal says about a place's PR: its number, and whether its
/// end (merged, closed) was journaled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Known {
    pub number: u64,
    pub ended: bool,
}

fn ended(s: PrState) -> bool {
    matches!(s, PrState::Merged | PrState::Closed)
}

fn end_event(place: &str, pr: &PrSnapshot) -> Option<PrEvent> {
    match pr.state {
        PrState::Merged => Some(PrEvent::Merged { place: place.into(), pr: pr.clone() }),
        PrState::Closed => Some(PrEvent::Closed { place: place.into(), pr: pr.clone() }),
        _ => None,
    }
}

/// Whether a PR the forge gives for a place's branch is the place's: a
/// merged or closed PR this place never knew, whose head is not the
/// branch's tip, is an older PR of a branch name used again (a new task
/// `x` on `sb/x` after an old one's PR was merged): not this place's.
pub fn belongs(pr: &PrSnapshot, known: Option<Known>, tip: Option<&str>) -> bool {
    !ended(pr.state) || known.is_some_and(|k| k.number == pr.number) || tip == Some(pr.head_oid.as_str())
}

/// The events between the last answer (`old`, this run of the hub) and
/// this one (`new`), given what the journal knows. A PR that disappears
/// (its branch renamed on the forge) makes no event.
pub fn diff(place: &str, old: Option<&PrSnapshot>, new: Option<&PrSnapshot>, known: Option<Known>) -> Vec<PrEvent> {
    let Some(new) = new else { return Vec::new() };
    let mut out = Vec::new();
    match old.filter(|o| o.number == new.number) {
        Some(old) => {
            if old != new {
                out.push(PrEvent::Changed { place: place.into(), from: old.clone(), to: new.clone() });
            }
            if ended(old.state) && !ended(new.state) {
                // reopened: seen again (the journal forgets its end)
                out.push(PrEvent::Seen { place: place.into(), pr: new.clone() });
            } else if !ended(old.state) {
                out.extend(end_event(place, new));
            }
        }
        None => match known.filter(|k| k.number == new.number) {
            // the hub restarted: the journal saw it; only an end missed
            Some(k) => {
                if !k.ended {
                    out.extend(end_event(place, new));
                } else if !ended(new.state) {
                    out.push(PrEvent::Seen { place: place.into(), pr: new.clone() });
                }
            }
            None => {
                out.push(PrEvent::Seen { place: place.into(), pr: new.clone() });
                out.extend(end_event(place, new));
            }
        },
    }
    out
}

/// The journal line of an event, if it is journaled (hub-owned: the
/// hub reads it back at its start, sb-core never sees it).
pub fn journal_line(e: &PrEvent) -> Option<serde_json::Value> {
    let (t, place, pr) = match e {
        PrEvent::Seen { place, pr } => ("pr_seen", place, pr),
        PrEvent::Merged { place, pr } => ("pr_merged", place, pr),
        PrEvent::Closed { place, pr } => ("pr_closed", place, pr),
        _ => return None,
    };
    Some(serde_json::json!({"type": t, "place": place, "branch": pr.branch, "number": pr.number}))
}

/// The hub.log line of an event.
pub fn log_line(e: &PrEvent) -> String {
    let at = |place: &str, pr: &PrSnapshot| format!("{} #{} ({})", place, pr.number, pr.branch);
    match e {
        PrEvent::Seen { place, pr } => format!("PR seen: {} {}", at(place, pr), pr.url),
        PrEvent::Changed { place, from, to } => format!(
            "PR changed: {}: {:?}/{:?}/{:?} -> {:?}/{:?}/{:?}",
            at(place, to),
            from.state,
            from.review,
            from.checks,
            to.state,
            to.review,
            to.checks
        ),
        PrEvent::Merged { place, pr } => format!("PR merged: {}", at(place, pr)),
        PrEvent::Closed { place, pr } => format!("PR closed: {}", at(place, pr)),
        PrEvent::Unreachable { error } => format!("PRs: the forge did not answer, backing off: {}", error.describe()),
    }
}

/// Whether a journal event is one of [`journal_line`]'s.
pub fn is_pr_line(ev: &serde_json::Value) -> bool {
    matches!(ev["type"].as_str(), Some("pr_seen" | "pr_merged" | "pr_closed"))
}

/// Read one of [`journal_line`]'s lines into what the journal knows.
pub fn read_line(known: &mut std::collections::BTreeMap<String, Known>, ev: &serde_json::Value) {
    let (Some(place), Some(number)) = (ev["place"].as_str(), ev["number"].as_u64()) else { return };
    match ev["type"].as_str() {
        Some("pr_seen") => {
            known.insert(place.into(), Known { number, ended: false });
        }
        Some("pr_merged" | "pr_closed") => {
            known.insert(place.into(), Known { number, ended: true });
        }
        _ => {}
    }
}

/// The held line of a worktree with no PR yet (pr-design §4.1 rule 5).
pub fn no_pr_lid(commits: u32) -> String {
    match commits {
        0 => "no PR yet".into(),
        1 => "no PR yet · 1 commit".into(),
        n => format!("no PR yet · {} commits", n),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::place::{Checks, Review};

    pub(crate) fn pr(n: u64, state: PrState) -> PrSnapshot {
        PrSnapshot {
            number: n,
            url: format!("https://github.com/o/r/pull/{}", n),
            branch: "sb/a".into(),
            head_oid: "h1".into(),
            state,
            review: Review::None,
            checks: Checks::None,
            updated_at: "t1".into(),
        }
    }

    fn kinds(es: &[PrEvent]) -> Vec<&'static str> {
        es.iter()
            .map(|e| match e {
                PrEvent::Seen { .. } => "seen",
                PrEvent::Changed { .. } => "changed",
                PrEvent::Merged { .. } => "merged",
                PrEvent::Closed { .. } => "closed",
                PrEvent::Unreachable { .. } => "unreachable",
            })
            .collect()
    }

    #[test]
    fn repo_urls() {
        let r = |u| repo_of_url(u).map(|r| format!("{} {} {}", r.host, r.owner, r.name));
        assert_eq!(r("https://github.com/gvergnaud/bise.git").as_deref(), Some("github.com gvergnaud bise"));
        assert_eq!(r("https://github.com/o/r").as_deref(), Some("github.com o r"));
        assert_eq!(r("git@github.com:o/r.git").as_deref(), Some("github.com o r"));
        assert_eq!(r("ssh://git@ghe.corp:2222/o/r.git").as_deref(), Some("ghe.corp o r"));
        assert_eq!(r("https://user@GitHub.com/o/r/").as_deref(), Some("github.com o r"));
        assert_eq!(r("/local/path/repo"), None);
        assert_eq!(r("https://gitlab.com/group/sub/r.git"), None);
    }

    #[test]
    fn the_events_of_a_pr_life() {
        let open = pr(7, PrState::Open);
        assert_eq!(kinds(&diff("p", None, Some(&open), None)), ["seen"]);
        assert_eq!(kinds(&diff("p", Some(&open), Some(&open), None)), Vec::<&str>::new());
        let mut red = open.clone();
        red.checks = Checks::Fail { failing: vec!["ci".into()] };
        assert_eq!(kinds(&diff("p", Some(&open), Some(&red), None)), ["changed"]);
        let merged = pr(7, PrState::Merged);
        assert_eq!(kinds(&diff("p", Some(&red), Some(&merged), None)), ["changed", "merged"]);
        // merged and seen again: nothing new
        assert_eq!(kinds(&diff("p", Some(&merged), Some(&merged), None)), Vec::<&str>::new());
        // no answer for the branch: nothing
        assert!(diff("p", Some(&open), None, None).is_empty());
        // a new PR on the same branch: seen
        assert_eq!(kinds(&diff("p", Some(&open), Some(&pr(8, PrState::Open)), None)), ["seen"]);
        // closed, then reopened
        let closed = pr(7, PrState::Closed);
        assert_eq!(kinds(&diff("p", Some(&open), Some(&closed), None)), ["changed", "closed"]);
        assert_eq!(kinds(&diff("p", Some(&closed), Some(&open), None)), ["changed", "seen"]);
    }

    #[test]
    fn a_restart_never_sees_a_pr_twice() {
        let open = pr(7, PrState::Open);
        let k = Some(Known { number: 7, ended: false });
        assert!(diff("p", None, Some(&open), k).is_empty());
        // merged while the hub was down
        assert_eq!(kinds(&diff("p", None, Some(&pr(7, PrState::Merged)), k)), ["merged"]);
        let k = Some(Known { number: 7, ended: true });
        assert!(diff("p", None, Some(&pr(7, PrState::Merged)), k).is_empty());
        // a first sight already merged: seen, then merged
        assert_eq!(kinds(&diff("p", None, Some(&pr(9, PrState::Merged)), None)), ["seen", "merged"]);
    }

    #[test]
    fn an_old_pr_of_a_reused_branch_name_is_not_the_place_s() {
        let old = pr(3, PrState::Merged);
        assert!(!belongs(&old, None, Some("fresh-tip")));
        assert!(belongs(&old, None, Some("h1")));
        assert!(belongs(&old, Some(Known { number: 3, ended: false }), None));
        assert!(belongs(&pr(3, PrState::Open), None, None));
    }

    #[test]
    fn the_journal_lines_round_trip() {
        let mut known = std::collections::BTreeMap::new();
        for e in [
            PrEvent::Seen { place: "wt:a".into(), pr: pr(7, PrState::Open) },
            PrEvent::Seen { place: "wt:b".into(), pr: pr(8, PrState::Open) },
            PrEvent::Merged { place: "wt:b".into(), pr: pr(8, PrState::Merged) },
        ] {
            let l = journal_line(&e).unwrap();
            assert!(is_pr_line(&l));
            read_line(&mut known, &l);
        }
        assert_eq!(known["wt:a"], Known { number: 7, ended: false });
        assert_eq!(known["wt:b"], Known { number: 8, ended: true });
        let changed = PrEvent::Changed { place: "p".into(), from: pr(7, PrState::Open), to: pr(7, PrState::Open) };
        assert_eq!(journal_line(&changed), None);
        assert_eq!(no_pr_lid(0), "no PR yet");
        assert_eq!(no_pr_lid(1), "no PR yet · 1 commit");
        assert_eq!(no_pr_lid(2), "no PR yet · 2 commits");
    }
}
