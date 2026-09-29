//! The processes an agent starts (BISE-243): its bash commands, their
//! background jobs, dev servers, test hubs, tmux servers. The hub kills
//! them when the agent is stopped or archived, when the hub quits for
//! good, and at its start for the agents that are gone
//! (docs/proc-cleanup.md).
//!
//! How they are tracked: each REPL gets `BISE_OWNERS`, a list of tags
//! `<hub>.<dir>.<spawn ms>` (the hub: a hash of its socket path). Every
//! process the agent starts inherits it, even one that leaves its
//! process group or session (`setsid`, `nohup`, a tmux server, a hub
//! that daemonizes) or is reparented to pid 1. A hub an agent starts
//! gives its own REPLs the list it inherited plus its own tag, so what
//! they start is the outer agent's too. A process chooses to outlive its
//! agent with `BISE_OWNERS=` (empty): `scripts/relaunch-live.sh` does.
//!
//! The list is read from the process table at kill time (macOS `ps -E`,
//! Linux `/proc/<pid>/environ`): a reused pid does not carry the tag, so
//! it is never hit. macOS hides the environment of its own binaries
//! (`/bin/sleep`, `/bin/sh`, `/usr/bin/python3`...): for those, the REPL
//! runs in its own session (`setsid`), each REPL's pid is kept in its
//! agent's folder (`repl.sids`), and a process with no visible list in
//! such a session is the agent's. A session whose leader is alive and
//! not that agent's tagged REPL (a reused pid) is ignored. A tagged process's children are the agent's too when
//! they carry no list at all (`env -i`); a child with another agent's
//! tags, or with an empty list, is not. The hub itself and its ancestors
//! are never killed (a hub an agent relaunched carries that agent's tag).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{Duration, Instant};

/// The variable every process started by an agent carries.
pub const ENV: &str = "BISE_OWNERS";

/// One hub's id in the tags: FNV-1a of its socket path, 16 hex digits.
pub fn hub_id(socket: &Path) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in socket.to_string_lossy().bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}

/// An agent's tag: `<hub>.<dir>.<ms>`, the dir kept to `[A-Za-z0-9_-]`
/// (the list is one word in `ps -E`).
pub fn tag(hub: &str, dir: &str, ms: u64) -> String {
    let d: String = dir
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    format!("{}.{}.{}", hub, d, ms)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    pub hub: String,
    pub dir: String,
    pub ms: u64,
}

pub fn parse_tag(s: &str) -> Option<Tag> {
    let (hub, rest) = s.split_once('.')?;
    let (dir, ms) = rest.rsplit_once('.')?;
    Some(Tag {
        hub: hub.to_string(),
        dir: dir.to_string(),
        ms: ms.parse().ok()?,
    })
}

fn split(list: &str) -> impl Iterator<Item = &str> {
    list.split(',').map(str::trim).filter(|t| !t.is_empty())
}

/// The list without this hub's own tags: a hub relaunched by one of its
/// agents is not that agent's (nor are the processes it starts).
pub fn without_hub(list: &str, hub: &str) -> String {
    split(list)
        .filter(|t| parse_tag(t).is_none_or(|t| t.hub != hub))
        .collect::<Vec<_>>()
        .join(",")
}

/// A REPL's list: what the hub inherited (not its own tags) + its tag.
pub fn for_repl(inherited: Option<&str>, hub: &str, dir: &str, ms: u64) -> String {
    let mut l = without_hub(inherited.unwrap_or(""), hub);
    if !l.is_empty() {
        l.push(',');
    }
    l.push_str(&tag(hub, dir, ms));
    l
}

/// One process of the table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proc {
    pub pid: u32,
    pub ppid: u32,
    /// its start time, as the system says it: with the pid, who it is
    pub start: String,
    pub zombie: bool,
    /// its session id (`getsid`)
    pub sid: u32,
    /// `BISE_OWNERS`: None when it has no such variable
    pub owners: Option<String>,
    pub command: String,
}

/// Which agents' processes to kill: this hub's tags whose dir is in
/// `dirs` (spawned before `before` ms), or, at the start, whose dir is
/// not one of the live agents'.
pub enum Want<'a> {
    Dirs { dirs: &'a BTreeSet<String>, before: u64 },
    NotLive(&'a BTreeSet<String>),
}

impl Want<'_> {
    fn hits(&self, hub: &str, owners: &str) -> bool {
        split(owners).filter_map(parse_tag).any(|t| {
            t.hub == hub
                && match self {
                    Want::Dirs { dirs, before } => t.ms <= *before && dirs.iter().any(|d| tag(hub, d, 0) == tag(hub, &t.dir, 0)),
                    Want::NotLive(live) => !live.iter().any(|d| tag(hub, d, 0) == tag(hub, &t.dir, 0)),
                }
        })
    }
}

/// The processes to kill, pure: the tagged ones, the ones with no
/// visible list in one of the agents' REPL `sessions`, and their
/// children that carry no list; never `me` nor its ancestors (nor
/// anything below `me` that is not tagged).
pub fn select(procs: &[Proc], hub: &str, want: &Want, sessions: &BTreeSet<u32>, me: u32) -> Vec<Proc> {
    let by_pid: BTreeMap<u32, &Proc> = procs.iter().map(|p| (p.pid, p)).collect();
    // a session is the agent's while its leader is gone (a zombie: the
    // REPL just killed) or is its REPL
    let sessions: BTreeSet<u32> = sessions
        .iter()
        .copied()
        .filter(|s| {
            *s > 1 && by_pid.get(s).filter(|l| !l.zombie).is_none_or(|l| l.owners.as_deref().is_some_and(|o| want.hits(hub, o)))
        })
        .collect();
    let seed = |p: &Proc| match p.owners.as_deref() {
        Some(o) => want.hits(hub, o),
        None => sessions.contains(&p.sid),
    };
    let mut protect: BTreeSet<u32> = [0, 1, me].into();
    let mut p = me;
    while let Some(pp) = by_pid.get(&p).map(|x| x.ppid) {
        if !protect.insert(pp) {
            break;
        }
        p = pp;
    }
    let mut children: BTreeMap<u32, Vec<&Proc>> = BTreeMap::new();
    for p in procs {
        children.entry(p.ppid).or_default().push(p);
    }
    let mut out: BTreeSet<u32> = BTreeSet::new();
    let mut todo: Vec<u32> = procs
        .iter()
        .filter(|p| !p.zombie && seed(p))
        .map(|p| p.pid)
        .collect();
    while let Some(pid) = todo.pop() {
        if protect.contains(&pid) || !out.insert(pid) {
            continue;
        }
        for c in children.get(&pid).into_iter().flatten() {
            let mine = match c.owners.as_deref() {
                None => true,
                Some(o) => want.hits(hub, o),
            };
            if mine && !c.zombie {
                todo.push(c.pid);
            }
        }
    }
    out.iter().filter_map(|p| by_pid.get(p).map(|x| (*x).clone())).collect()
}

/// `ps -axww -E -o pid=,ppid=,stat=,lstart=,command=` (macOS): the
/// environment comes after the command, one `K=V` word each; the last
/// ` BISE_OWNERS=` word is the variable (a command naming it comes first).
pub fn parse_ps(text: &str) -> Vec<Proc> {
    let key = format!(" {}=", ENV);
    text.lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let pid = w.next()?.parse().ok()?;
            let ppid = w.next()?.parse().ok()?;
            let stat = w.next()?;
            let start: Vec<&str> = w.by_ref().take(5).collect();
            if start.len() < 5 {
                return None;
            }
            let command = w.next().unwrap_or("").to_string();
            let owners = l.rfind(&key).map(|i| {
                l[i + key.len()..].split_whitespace().next().filter(|v| !v.contains('=')).unwrap_or("").to_string()
            });
            Some(Proc {
                pid,
                ppid,
                start: start.join(" "),
                zombie: stat.starts_with('Z'),
                sid: 0,
                owners,
                command,
            })
        })
        .collect()
}

/// The process table of this user, with each process's `BISE_OWNERS`.
pub fn snapshot() -> Vec<Proc> {
    #[cfg(target_os = "linux")]
    {
        linux_snapshot()
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::process::Command::new("ps")
            .args(["-axww", "-E", "-o", "pid=,ppid=,stat=,lstart=,command="])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .map(|o| parse_ps(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default()
            .into_iter()
            .map(|mut p| {
                // SAFETY: getsid(2) on a pid; a stale one returns -1
                p.sid = unsafe { getsid(p.pid as i32) }.max(0) as u32;
                p
            })
            .collect()
    }
}

#[cfg(target_os = "linux")]
fn linux_snapshot() -> Vec<Proc> {
    let Ok(rd) = std::fs::read_dir("/proc") else {
        return vec![];
    };
    rd.flatten()
        .filter_map(|e| {
            let pid: u32 = e.file_name().to_str()?.parse().ok()?;
            let stat = std::fs::read_to_string(e.path().join("stat")).ok()?;
            let (head, rest) = stat.rsplit_once(')')?;
            let f: Vec<&str> = rest.split_whitespace().collect();
            let env = std::fs::read(e.path().join("environ")).ok()?;
            let key = format!("{}=", ENV);
            let owners = env
                .split(|b| *b == 0)
                .filter_map(|kv| std::str::from_utf8(kv).ok())
                .find_map(|kv| kv.strip_prefix(&key).map(str::to_string));
            Some(Proc {
                pid,
                ppid: f.get(1)?.parse().ok()?,
                start: f.get(19)?.to_string(),
                zombie: f.first() == Some(&"Z"),
                sid: f.get(3)?.parse().ok()?,
                owners,
                command: head.split_once('(').map(|(_, c)| c.to_string()).unwrap_or_default(),
            })
        })
        .collect()
}

extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
    #[cfg(not(target_os = "linux"))]
    fn getsid(pid: i32) -> i32;
    fn setsid() -> i32;
}

/// Run `cmd` in a session of its own (a REPL: its session id is its pid).
pub fn own_session(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    // SAFETY: setsid is async-signal-safe; nothing else runs between
    // fork and exec
    unsafe {
        cmd.pre_exec(|| {
            setsid();
            Ok(())
        });
    }
}

/// The REPL session ids kept for an agent (`repl.sids`, one per line).
pub fn read_sids(file: &Path) -> BTreeSet<u32> {
    std::fs::read_to_string(file)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .collect()
}

pub fn add_sid(file: &Path, pid: u32) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(file) {
        let _ = writeln!(f, "{}", pid);
    }
}

const SIGTERM: i32 = 15;
const SIGKILL: i32 = 9;

fn signal(p: &Proc, sig: i32) {
    // SAFETY: kill(2) with a pid from the table; a stale pid fails
    unsafe {
        kill(p.pid as i32, sig);
    }
}

/// Kill what `want` names: SIGTERM, up to `grace` for them to go, then
/// SIGKILL to the ones still there (same pid, same start time). Returns
/// the processes it signalled.
pub fn reap(hub: &str, want: &Want, sessions: &BTreeSet<u32>, grace: Duration) -> Vec<Proc> {
    let me = std::process::id();
    let hit = select(&snapshot(), hub, want, sessions, me);
    if hit.is_empty() {
        return hit;
    }
    for p in &hit {
        signal(p, SIGTERM);
    }
    let t0 = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(100));
        let now: BTreeSet<(u32, String)> = snapshot()
            .into_iter()
            .filter(|p| !p.zombie)
            .map(|p| (p.pid, p.start))
            .collect();
        let left: Vec<&Proc> = hit.iter().filter(|p| now.contains(&(p.pid, p.start.clone()))).collect();
        if left.is_empty() {
            break;
        }
        if t0.elapsed() >= grace {
            for p in left {
                signal(p, SIGKILL);
            }
            break;
        }
    }
    hit
}

/// One line for the hub log: `3 (1234 sleep, 1240 bise, ...)`.
pub fn describe(hit: &[Proc]) -> String {
    let names: Vec<String> = hit
        .iter()
        .take(12)
        .map(|p| format!("{} {}", p.pid, Path::new(&p.command).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()))
        .collect();
    format!("{} ({}{})", hit.len(), names.join(", "), if hit.len() > 12 { ", ..." } else { "" })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pid: u32, ppid: u32, owners: Option<&str>) -> Proc {
        Proc {
            pid,
            ppid,
            start: format!("s{}", pid),
            zombie: false,
            sid: pid,
            owners: owners.map(str::to_string),
            command: format!("/bin/p{}", pid),
        }
    }

    fn pids(v: Vec<Proc>) -> Vec<u32> {
        v.into_iter().map(|p| p.pid).collect()
    }

    /// A REPL's list keeps the outer hubs' tags, drops its own hub's
    /// (a hub an agent relaunched), and adds its tag.
    #[test]
    fn a_repl_list_nests_the_outer_agents() {
        let h = "00000000000000aa";
        assert_eq!(for_repl(None, h, "t1", 5), "00000000000000aa.t1.5");
        assert_eq!(
            for_repl(Some("bb.x.1,00000000000000aa.main.2"), h, "t 1.b", 5),
            "bb.x.1,00000000000000aa.t_1_b.5"
        );
        assert_eq!(parse_tag("aa.t_1.b.7"), Some(Tag { hub: "aa".into(), dir: "t_1.b".into(), ms: 7 }));
        assert_eq!(hub_id(Path::new("/a/hub.sock")).len(), 16);
        assert_ne!(hub_id(Path::new("/a/hub.sock")), hub_id(Path::new("/b/hub.sock")));
    }

    /// The tagged processes of the dropped agent go, with their children
    /// that carry no list; not another agent's, not a process that opted
    /// out, not a later REPL of the same agent, never the hub or its
    /// ancestors, and never a pid whose process has no tag.
    #[test]
    fn select_takes_the_agent_s_tree_only() {
        let h = "aa";
        let procs = vec![
            p(1, 0, None),
            p(50, 1, Some("zz.agent.1")),          // an agent of an outer hub
            p(100, 50, Some("zz.agent.1,aa.t1.3")), // the hub, relaunched by t1
            p(101, 100, Some("aa.t1.10")),         // t1's REPL
            p(102, 101, Some("aa.t1.10")),         // its bash
            p(103, 102, None),                     // env -i under it
            p(104, 1, Some("aa.t1.10,cc.x.1")),    // t1's test hub REPL, reparented
            p(105, 102, Some("")),                 // opted out
            p(106, 105, None),                     // under the opted-out one
            p(107, 101, Some("aa.t2.10")),         // t2's (odd parent)
            p(108, 100, Some("aa.t1.99")),         // t1 respawned after the drop
            p(109, 1, None),                       // the user's
        ];
        let dirs: BTreeSet<String> = ["t1".to_string()].into();
        let none = BTreeSet::new();
        let got = pids(select(&procs, h, &Want::Dirs { dirs: &dirs, before: 50 }, &none, 100));
        assert_eq!(got, vec![101, 102, 103, 104]);
        // at the start: every agent that is not live (t2 is)
        let live: BTreeSet<String> = ["t2".to_string()].into();
        assert_eq!(pids(select(&procs, h, &Want::NotLive(&live), &none, 100)), vec![101, 102, 103, 104, 108]);
        // another hub's agent named t1 is not this one
        assert!(select(&procs, "dd", &Want::Dirs { dirs: &dirs, before: 50 }, &none, 100).is_empty());
    }

    /// macOS hides the environment of its own binaries: in a REPL's
    /// session, a process with no visible list is the agent's; not when
    /// the session's leader is alive and someone else (a reused pid).
    #[test]
    fn select_takes_the_hidden_ones_of_the_repl_session() {
        let h = "aa";
        let mut s1 = p(201, 1, None); // /bin/sleep, orphaned, in 101's session
        s1.sid = 101;
        let mut s2 = p(202, 1, None); // in 300's session (leader: the user's)
        s2.sid = 300;
        let mut s3 = p(203, 1, Some("")); // opted out, in 101's session
        s3.sid = 101;
        let mut s4 = p(204, 1, None); // in 400's session (leader gone)
        s4.sid = 400;
        let procs = vec![p(100, 1, None), p(101, 100, Some("aa.t1.10")), p(300, 1, Some("zz.u.1")), s1, s2, s3, s4];
        let dirs: BTreeSet<String> = ["t1".to_string()].into();
        let want = Want::Dirs { dirs: &dirs, before: 50 };
        let sids: BTreeSet<u32> = [101, 300, 400].into();
        assert_eq!(pids(select(&procs, h, &want, &sids, 100)), vec![101, 201, 204]);
        assert_eq!(pids(select(&procs, h, &want, &[1u32].into(), 100)), vec![101]);
        // the REPL just killed, a zombie (no list shown): its session still counts
        let mut procs = procs;
        procs[1].zombie = true;
        procs[1].owners = None;
        assert_eq!(pids(select(&procs, h, &want, &sids, 100)), vec![201, 204]);
    }

    #[test]
    fn ps_lines_give_the_list() {
        let text = "  101   100 S    Tue Oct  7 00:48:12 2026 /x/repl-live A=1 BISE_OWNERS=aa.t1.3 B=2\n\
                    102 101 Z+ Tue Oct  7 00:48:13 2026 rg BISE_OWNERS=aa.t9.1 x HOME=/h BISE_OWNERS=aa.t1.3\n\
                    103 1 S Tue Oct  7 00:48:13 2026 sleep 5 HOME=/h BISE_OWNERS= TERM=x\n\
                    104 1 S Tue Oct  7 00:48:13 2026 sleep 5 HOME=/h\n";
        let v = parse_ps(text);
        assert_eq!(v.len(), 4);
        assert_eq!(v[0].owners.as_deref(), Some("aa.t1.3"));
        assert_eq!(v[0].start, "Tue Oct 7 00:48:12 2026");
        assert_eq!(v[0].command, "/x/repl-live");
        assert!(v[1].zombie && v[1].owners.as_deref() == Some("aa.t1.3"));
        assert_eq!(v[2].owners.as_deref(), Some(""));
        assert_eq!(v[3].owners, None);
    }
}
