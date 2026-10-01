//! Places (dev-flow §3.1): where agents work, shared, not owned. The
//! shared folder is one place; each worktree (a folder and its branch)
//! is another, which several agents may join (`sb spawn --place`, `sb
//! move`). A PR belongs to a place's branch, never to an agent.
//!
//! Storage: a place is the `place` id in the `ws` of each agent in it
//! (journaled by sb-core with the rest of the agent, `Workspace::place`);
//! this module derives the table from the state. An older journal's
//! worktree has no id: it is its agent's own place (`wt:<dir>`), so the
//! journal needs no migration.
//!
//! THE CONTRACT (frozen by wave 1, docs/pr-briefs.md §Contracts): the
//! types below and their JSON (serde) are what wave 2 builds on:
//! pr-hub fills [`Place::pr`] (`places(st, prs)`), pr-tui reads
//! `places: [PlaceView]` in the TUI snapshot and `place_id` on each
//! agent. Change them only with main and the wave 2 agents.
//!
//! BISE-136 (designer's call 8) adds a kind of id, not a type: a private
//! worktree an agent told the hub about (`gate.sh new`) is a worktree
//! place `pt:<path>` ([`private_id`]), its branch what it has checked
//! out (None: detached, the views name it by its folder).

use crate::model::{Lifecycle, Mode, State};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The id of the shared folder's place.
pub const SHARED: &str = "shared";

/// The id of the worktree first made for the agent of dir `dir`.
pub fn worktree_id(dir: &str) -> String {
    format!("wt:{}", dir)
}

/// BISE-136, designer's call 8: the id of a private worktree an agent
/// told the hub about (`gate.sh new`, `sb worktree <path>`): `pt:<path>`.
/// It is a worktree like any other (a row with its mark alone, a box
/// shared), with no branch when detached: the views then name it by its
/// folder, the path's last part.
pub fn private_id(path: &str) -> String {
    format!("pt:{}", path)
}

/// The path of a private worktree's id (None: not one).
pub fn private_path(id: &str) -> Option<&str> {
    id.strip_prefix("pt:")
}

/// The id of the place agent `a` works in: its private worktree when it
/// told one (live agents only), else its workspace's place.
pub fn id_of(a: &crate::model::Agent) -> String {
    match &a.place {
        Some(p) if a.lifecycle != Lifecycle::Archived => private_id(p),
        _ => a.ws.place_id(&a.dir),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaceKind {
    Shared,
    Worktree,
}

/// A place (dev-flow §3.1). `agents`: the agents in it, not archived,
/// in the hub's order (the first one names the box). `base`: the commit
/// the worktree was made from. `pr`: the PR of its branch (pr-hub, wave
/// 2; None until then).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    pub id: String,
    pub kind: PlaceKind,
    pub path: String,
    pub branch: Option<String>,
    pub base: Option<String>,
    pub agents: Vec<String>,
    pub pr: Option<PrSnapshot>,
}

/// pr-design §9: what the forge says about the PR whose head is a
/// place's branch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrSnapshot {
    pub number: u64,
    pub url: String,
    pub branch: String,
    pub head_oid: String,
    pub state: PrState,
    pub review: Review,
    pub checks: Checks,
    /// The forge's `updatedAt` (ISO 8601).
    pub updated_at: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrState {
    Draft,
    Open,
    Merged,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Review {
    None,
    Pending,
    Approved,
    ChangesRequested,
}

/// JSON: `{"state": "pass"}`, `{"state": "fail", "failing": ["ci/test"]}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Checks {
    None,
    Running,
    Pass,
    Fail { failing: Vec<String> },
}

/// A place as the TUI draws it (pr-design §4.1): the snapshot's
/// `places`, worktrees only (the shared folder is never a box), in the
/// order of their first agent. `lid`: the held line under the border
/// (`waits to land · 2nd`; pr-hub adds the PR's), None when nothing to say.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceView {
    pub id: String,
    pub branch: Option<String>,
    pub agents: Vec<String>,
    pub pr: Option<PrView>,
    pub lid: Option<String>,
}

/// The PR as the TUI shows it. `stale_ms`: how old the last answer of
/// the forge is, when it is late (offline, rate limit); None when fresh.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrView {
    pub number: u64,
    pub url: String,
    pub state: PrState,
    pub review: Review,
    pub checks: Checks,
    pub stale_ms: Option<u64>,
}

/// The places of a state: the shared folder first (always), then each
/// worktree with a live folder, in the order of its first agent. A
/// worktree whose agents are all archived stays while its folder does
/// (a drop that could not remove it, or, from wave 2, an open PR),
/// with no agents. `prs`: the PR of each place id (pr-hub).
pub fn places(st: &State, prs: &BTreeMap<String, PrSnapshot>) -> Vec<Place> {
    let mut out = vec![Place {
        id: SHARED.to_string(),
        kind: PlaceKind::Shared,
        path: st.agents.get(crate::model::MAIN).map(|a| a.ws.path.clone()).unwrap_or_default(),
        branch: None,
        base: None,
        agents: Vec::new(),
        pr: prs.get(SHARED).cloned(),
    }];
    for a in st.order.iter().filter_map(|n| st.agents.get(n)) {
        let live = a.lifecycle != Lifecycle::Archived;
        if a.ws.mode == Mode::Worktree && a.ws.dropped {
            continue;
        }
        let id = a.ws.place_id(&a.dir);
        let i = match out.iter().position(|p| p.id == id) {
            Some(i) => i,
            None => {
                out.push(Place {
                    id: id.clone(),
                    kind: PlaceKind::Worktree,
                    path: a.ws.path.clone(),
                    branch: a.ws.branch.clone(),
                    base: a.ws.base_commit.clone(),
                    agents: Vec::new(),
                    pr: prs.get(&id).cloned(),
                });
                out.len() - 1
            }
        };
        // BISE-136: an agent in a private worktree is in that place (its
        // workspace's worktree stays, for its folder and its PR)
        let i = match a.place.as_ref().filter(|_| live) {
            Some(path) => {
                let pid = private_id(path);
                match out.iter().position(|p| p.id == pid) {
                    Some(j) => j,
                    None => {
                        out.push(Place {
                            id: pid.clone(),
                            kind: PlaceKind::Worktree,
                            path: path.clone(),
                            branch: a.place_branch.clone(),
                            base: None,
                            agents: Vec::new(),
                            pr: prs.get(&pid).cloned(),
                        });
                        out.len() - 1
                    }
                }
            }
            None => i,
        };
        if live {
            out[i].agents.push(a.name.clone());
        }
    }
    out
}

/// The worktree a task joins (`sb spawn --place`, `sb move`): the one of
/// agent `target` (a name or an old name), or the live place whose
/// branch is `target`. Its `ws`, with the place id set.
pub fn find_worktree(st: &State, target: &str) -> Result<crate::model::Workspace, String> {
    let live = |a: &&crate::model::Agent| {
        a.ws.mode == Mode::Worktree && !a.ws.dropped && a.lifecycle != Lifecycle::Archived
    };
    let by_agent = st.resolve(target).and_then(|n| st.agents.get(&n));
    if let Some(a) = by_agent {
        if !live(&a) {
            return Err(format!(
                "@{} works in the shared folder, not a worktree: `--place new` makes one",
                a.name
            ));
        }
    }
    let a = by_agent
        .or_else(|| {
            st.order
                .iter()
                .filter_map(|n| st.agents.get(n))
                .filter(live)
                .find(|a| a.ws.branch.as_deref() == Some(target))
        })
        .ok_or_else(|| format!("no place {}: name an agent in a worktree, or its branch", target))?;
    let mut ws = a.ws.clone();
    ws.place = Some(a.ws.place_id(&a.dir));
    Ok(ws)
}

/// The place of agent `name` (its id), if it has one.
pub fn place_of(st: &State, name: &str) -> Option<String> {
    st.agents.get(name).map(|a| a.ws.place_id(&a.dir))
}

/// The views the TUI draws: worktrees only. `lids`: the held line of a
/// place, by id (the land queue's `waits to land · 2nd`, else pr-hub's
/// `no PR yet · 2 commits`). `stale`: the age of a PR's state, by place
/// id, when the forge is late (pr-hub: its last ask failed, or it is
/// older than `core::PR_STALE_MS`).
pub fn views(places: &[Place], lids: &BTreeMap<String, String>, stale: &BTreeMap<String, u64>) -> Vec<PlaceView> {
    places
        .iter()
        .filter(|p| p.kind == PlaceKind::Worktree)
        .map(|p| PlaceView {
            id: p.id.clone(),
            branch: p.branch.clone(),
            agents: p.agents.clone(),
            pr: p.pr.as_ref().map(|pr| PrView {
                number: pr.number,
                url: pr.url.clone(),
                state: pr.state,
                review: pr.review,
                checks: pr.checks.clone(),
                stale_ms: stale.get(&p.id).copied(),
            }),
            lid: lids.get(&p.id).cloned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Workspace;

    fn wt(id: Option<&str>, path: &str, branch: &str) -> Workspace {
        Workspace {
            mode: Mode::Worktree,
            path: path.into(),
            branch: Some(branch.into()),
            base_commit: Some("abc".into()),
            dropped: false,
            place: id.map(Into::into),
        }
    }

    #[test]
    fn a_shared_worktree_is_one_place_with_its_agents_in_order() {
        let mut st = State::new("/w");
        for n in ["a", "b", "c", "d"] {
            st.test_task(n, "x");
        }
        st.agents.get_mut("a").unwrap().ws = wt(Some("wt:a"), "/wt/a", "sb/a");
        st.agents.get_mut("c").unwrap().ws = wt(Some("wt:a"), "/wt/a", "sb/a");
        // an older journal's worktree: no id, its agent's own place
        st.agents.get_mut("d").unwrap().ws = wt(None, "/wt/d", "sb/d");
        let ps = places(&st, &BTreeMap::new());
        let ids: Vec<&str> = ps.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["shared", "wt:a", "wt:d"]);
        assert_eq!(ps[0].kind, PlaceKind::Shared);
        assert_eq!(ps[0].agents, ["main", "b"]);
        assert_eq!(ps[1].agents, ["a", "c"]);
        assert_eq!(ps[1].branch.as_deref(), Some("sb/a"));
        assert_eq!(ps[2].agents, ["d"]);
        assert_eq!(place_of(&st, "c").as_deref(), Some("wt:a"));
        assert_eq!(place_of(&st, "b").as_deref(), Some("shared"));
        // an archived agent leaves the box; a dropped worktree is no place
        st.agents.get_mut("a").unwrap().lifecycle = Lifecycle::Archived;
        st.agents.get_mut("d").unwrap().ws.dropped = true;
        let ps = places(&st, &BTreeMap::new());
        assert_eq!(ps.len(), 2);
        assert_eq!(ps[1].agents, ["c"]);
        // the views: worktrees only, the lid by id
        let lids = BTreeMap::from([("wt:a".to_string(), "waits to land · 2nd".to_string())]);
        let v = views(&ps, &lids, &BTreeMap::new());
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].lid.as_deref(), Some("waits to land · 2nd"));
        assert_eq!(v[0].pr, None);
    }

    /// BISE-136, call 8: agents in a private worktree are in its place
    /// (`pt:<path>`, a worktree, its branch the one read there), shared
    /// when two are; an archived one leaves it; an agent of a hub
    /// worktree that works in a private one leaves its box, the box
    /// stays (its folder, its PR).
    #[test]
    fn a_private_worktree_is_a_place() {
        let mut st = State::new("/w");
        for n in ["a", "b", "c", "d"] {
            st.test_task(n, "x");
        }
        st.agents.get_mut("a").unwrap().place = Some("/p/a-wt".into());
        st.agents.get_mut("c").unwrap().place = Some("/p/a-wt".into());
        st.agents.get_mut("c").unwrap().place_branch = Some("feat/x".into());
        st.agents.get_mut("d").unwrap().ws = wt(Some("wt:d"), "/wt/d", "sb/d");
        st.agents.get_mut("d").unwrap().place = Some("/p/d-wt".into());
        let ps = places(&st, &BTreeMap::new());
        let ids: Vec<&str> = ps.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["shared", "pt:/p/a-wt", "wt:d", "pt:/p/d-wt"]);
        assert_eq!(ps[0].agents, ["main", "b"]);
        assert_eq!((ps[1].kind, ps[1].path.as_str()), (PlaceKind::Worktree, "/p/a-wt"));
        assert_eq!(ps[1].agents, ["a", "c"]);
        // the first agent's read names it (they read the same folder)
        assert_eq!(ps[1].branch, None);
        assert!(ps[2].agents.is_empty() && ps[3].agents == ["d"]);
        assert_eq!(id_of(&st.agents["c"]), "pt:/p/a-wt");
        assert_eq!(private_path("pt:/p/a-wt"), Some("/p/a-wt"));
        assert_eq!(private_path("wt:d"), None);
        // archived: out of it, its id is its workspace's again
        st.agents.get_mut("a").unwrap().lifecycle = Lifecycle::Archived;
        let ps = places(&st, &BTreeMap::new());
        assert_eq!(ps[1].agents, ["c"]);
        assert_eq!(ps[1].branch.as_deref(), Some("feat/x"));
        assert_eq!(id_of(&st.agents["a"]), "shared");
        // the views: a worktree like any other
        let v = views(&ps, &BTreeMap::new(), &BTreeMap::new());
        assert!(v.iter().any(|p| p.id == "pt:/p/a-wt" && p.agents == ["c"]));
    }

    #[test]
    fn the_contract_json() {
        let pr = PrSnapshot {
            number: 412,
            url: "https://github.com/o/r/pull/412".into(),
            branch: "sb/a".into(),
            head_oid: "abc".into(),
            state: PrState::Draft,
            review: Review::ChangesRequested,
            checks: Checks::Fail { failing: vec!["ci/test".into()] },
            updated_at: "2026-10-01T10:00:00Z".into(),
        };
        let v = PlaceView {
            id: "wt:a".into(),
            branch: Some("sb/a".into()),
            agents: vec!["a".into()],
            pr: Some(PrView {
                number: pr.number,
                url: pr.url.clone(),
                state: pr.state,
                review: pr.review,
                checks: pr.checks.clone(),
                stale_ms: None,
            }),
            lid: None,
        };
        let j = serde_json::to_value(&v).unwrap();
        assert_eq!(
            j,
            serde_json::json!({"id": "wt:a", "branch": "sb/a", "agents": ["a"], "lid": null,
                "pr": {"number": 412, "url": "https://github.com/o/r/pull/412", "state": "draft",
                       "review": "changes_requested", "checks": {"state": "fail", "failing": ["ci/test"]},
                       "stale_ms": null}})
        );
        assert_eq!(serde_json::to_value(Checks::Pass).unwrap(), serde_json::json!({"state": "pass"}));
        let back: PrSnapshot = serde_json::from_value(serde_json::to_value(&pr).unwrap()).unwrap();
        assert_eq!(back, pr);
    }
}
