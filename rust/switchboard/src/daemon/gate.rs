//! The approvals gate in the hub (approvals-design.md §8-§11, spec §3).
//!
//! The runtime asks, the hub decides. In `auto`, before a gated call, the
//! runtime prints `gate <n> {"tool","args","nonce"}` and waits on its gate
//! file (`run/bend-gate-<port>.txt`) and its interrupt file; the hub
//! appends `<n> <nonce> allow` or `<n> <nonce> deny <reason>` there. The
//! runtime prints `gate-done <n> <how>` when it goes on.
//!
//! Here: the mode (config.toml, `BISE_APPROVALS`, the mode files), then
//! for each gate line `judge` (tiers 0-4), the checker (tier 5, on its own
//! thread), or a `confirm` card in the user's inbox; the card's answer
//! writes the verdict, saves the rule on "always", and folds the card.
//! Identical calls from several agents share one card.
//!
//! The feed lines the TUI reads (`sb gate : …`, `sb approval : …`) are
//! [`gate_line`] and [`fold_line`].

use super::{log_line, Msg, Shell};
use crate::approvals::check::{CheckOut, CheckReq, Runner};
use crate::approvals::{self, mode, rules, Cache, CacheKey, Call, Checker, Mode, Part, Verdict};
use crate::core::Input;
use crate::model::MAIN;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The gate's state in the hub.
pub(super) struct Gates {
    pub(super) mode: Mode,
    /// `BISE_APPROVALS` set: a switch is for this session only.
    pub(super) env: bool,
    pub(super) runner: Arc<Runner>,
    /// The checker's verdicts, per git common root (design §4.4).
    caches: BTreeMap<PathBuf, Cache>,
    /// Each agent's git common root, by dir (asked once).
    repos: BTreeMap<String, PathBuf>,
    /// The saved rules and the file's mtime when read.
    rules: Option<(Option<std::time::SystemTime>, rules::Rules)>,
    /// The call each agent waits on, by dir.
    waiting: BTreeMap<String, Waiting>,
    /// The open cards of the gate, by card id.
    cards: BTreeMap<u64, GateCard>,
}

#[derive(Clone, Debug)]
struct Waiting {
    n: String,
    nonce: String,
    file: PathBuf,
    agent: String,
    card: Option<u64>,
}

#[derive(Clone, Debug)]
struct GateCard {
    /// Identical calls share a card: tool, arguments, repo.
    same: String,
    /// The agents (dirs) whose call waits on it, first the one it opened for.
    dirs: Vec<String>,
    /// The rules "always" saves; None: a hard rule, no option 2.
    always: Option<Vec<String>>,
    keys: Vec<CacheKey>,
    tool: String,
    repo: PathBuf,
    /// One line: what the fold says.
    summary: String,
}

impl Gates {
    pub(super) fn new(config: &str, env: Option<&str>, runner: Runner) -> Gates {
        let (mode, env) = mode::resolve(config, env);
        Gates {
            mode,
            env,
            runner: Arc::new(runner),
            caches: BTreeMap::new(),
            repos: BTreeMap::new(),
            rules: None,
            waiting: BTreeMap::new(),
            cards: BTreeMap::new(),
        }
    }

    /// What the TUI shows (the key bar, `/approvals`).
    pub(super) fn info(&self, bise: &Path) -> Value {
        let rules = rules::load(&rules::file(bise)).unwrap_or_default();
        let list: Vec<Value> = rules
            .rules
            .iter()
            .map(|r| {
                json!({
                    "tool": r.tool,
                    "pattern": r.pattern,
                    "path": r.path.as_ref().map(|p| p.display().to_string()),
                    "project": r.project.as_ref().map(|p| p.display().to_string()),
                })
            })
            .collect();
        let checker = match self.runner.checker() {
            Checker::Jev => "jev",
            Checker::Model => "model",
            Checker::Off => "off",
        };
        let who = self.runner.route().who();
        json!({"mode": self.mode.word(), "env": self.env, "checker": checker, "checker_who": who, "rules": list})
    }
}

/// A gate line of the runtime: `<n> <json>`.
pub(super) fn parse_gate(rest: &str) -> Option<(String, Value)> {
    let (n, j) = rest.split_once(' ')?;
    let v: Value = serde_json::from_str(j).ok()?;
    (!n.is_empty() && v.get("tool").is_some()).then(|| (n.to_string(), v))
}

/// The verdict line for the gate file (one line: a reason's newlines are
/// spaces).
pub(super) fn verdict_line(n: &str, nonce: &str, allow: bool, reason: &str) -> String {
    if allow {
        format!("{n} {nonce} allow\n")
    } else {
        let r: String = reason.split_whitespace().collect::<Vec<_>>().join(" ");
        format!("{n} {nonce} deny {r}\n")
    }
}

/// A gate state line of an agent's feed: `sb gate : check <n>` (the
/// checker runs), `card <n> <id>` (it waits on you), `done <n>`.
pub(super) fn gate_line(what: &str, n: &str, card: Option<u64>) -> String {
    match card {
        Some(c) => format!("sb gate : {what} {n} {c}"),
        None => format!("sb gate : {what} {n}"),
    }
}

/// The folded card (design §9): `sb approval : allowed : <agent> :
/// <summary>` or `no : <agent> : <summary> : <note>` (fields escaped).
pub(super) fn fold_line(allowed: bool, agents: &[String], summary: &str, note: &str) -> String {
    let esc = |s: &str| crate::util::one_line(s).replace(" : ", " \\: ");
    let who = agents.join(", ");
    if allowed {
        format!("sb approval : allowed : {} : {}", esc(&who), esc(summary))
    } else {
        format!("sb approval : no : {} : {} : {}", esc(&who), esc(summary), esc(note))
    }
}

/// What the user's answer says: allow, allow always, or no with a note.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Answer {
    Allow,
    Always,
    No(String),
}

pub(super) fn answer_of(text: &str) -> Answer {
    let t = text.trim();
    let low = t.to_lowercase();
    match low.as_str() {
        "1" | "allow" | "allow once" | "y" | "yes" | "ok" | "oui" => Answer::Allow,
        "2" | "always" | "always allow" | "always here" => Answer::Always,
        l if l.starts_with("always allow ") => Answer::Always,
        "3" | "no" | "n" | "deny" | "non" => Answer::No(String::new()),
        _ => {
            let note = ["no:", "deny:", "no -", "no,"]
                .iter()
                .find_map(|p| low.starts_with(p).then(|| t[p.len()..].trim().to_string()))
                .unwrap_or_else(|| t.to_string());
            Answer::No(note)
        }
    }
}

/// What the agent reads when the user says no.
pub(super) fn no_reason(note: &str) -> String {
    if note.is_empty() {
        "the user said no.".into()
    } else {
        format!("the user said no: {note}")
    }
}

/// A call's arguments as the gate line carries them (the model's text):
/// JSON when they parse, and a bash command given bare is `{"arg": cmd}`.
pub(super) fn args_of(tool: &str, v: Option<&Value>) -> Value {
    let v = match v {
        Some(Value::String(s)) => serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.clone())),
        Some(x) => x.clone(),
        None => Value::Null,
    };
    match v {
        Value::String(s) if tool == "bash" => json!({ "arg": s }),
        v => v,
    }
}

/// The command of a bash call, or "".
fn bash_cmd(args: &Value) -> String {
    args.get("arg")
        .or_else(|| args.get("command"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// The card's text (design §9), line by line so the TUI draws it and
/// main's board reads it: the title, `| ` the lines that need you, `= `
/// the parts already allowed (dim), `reason: `, one `always: ` per rule.
pub(super) fn card_text(call: &Call, needs: &[Part], reason: &str, always: Option<&[String]>) -> String {
    let mut out: Vec<String> = Vec::new();
    let body: Vec<String> = match call.tool.as_str() {
        "bash" => {
            let cmd = bash_cmd(&call.args);
            let all = approvals::parse::parse(&cmd).parts;
            let need: Vec<String> = needs.iter().map(|p| p.text()).collect();
            out.push("wants to run".into());
            if need.is_empty() {
                cmd.lines().map(str::to_string).collect()
            } else {
                for p in all.iter().map(|p| p.text()).filter(|t| !need.contains(t)) {
                    out.push(format!("= {}", crate::util::one_line(&p)));
                }
                need.iter().flat_map(|t| t.lines().map(str::to_string).collect::<Vec<_>>()).collect()
            }
        }
        t if rules::EDIT_TOOLS.contains(&t) => {
            let paths = approvals::edit_paths(t, &call.args);
            let outside = !paths.is_empty()
                && paths.iter().any(|p| {
                    let abs = if Path::new(p).is_absolute() { PathBuf::from(p) } else { call.cwd.join(p) };
                    !abs.starts_with(&call.cwd)
                });
            out.push(if outside { "wants to edit a file outside the repo".into() } else { "wants to edit".into() });
            let mut b: Vec<String> = paths.clone();
            b.extend(edit_diff(t, &call.args));
            b
        }
        t => {
            out.push(format!("wants to call {t}"));
            serde_json::to_string_pretty(&call.args).unwrap_or_default().lines().map(str::to_string).collect()
        }
    };
    for l in body.iter().take(40) {
        out.push(format!("| {l}"));
    }
    if body.len() > 40 {
        out.push(format!("| … {} more lines", body.len() - 40));
    }
    out.push(format!("reason: {reason}"));
    for r in always.into_iter().flatten() {
        out.push(format!("always: {r}"));
    }
    out.join("\n")
}

/// The lines an edit adds and removes, as a diff.
fn edit_diff(tool: &str, args: &Value) -> Vec<String> {
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    match tool {
        "edit" => {
            let mut v: Vec<String> = s("old_string").lines().map(|l| format!("-{l}")).collect();
            v.extend(s("new_string").lines().map(|l| format!("+{l}")));
            v
        }
        "write_file" => s("content").lines().map(|l| format!("+{l}")).collect(),
        _ => {
            let p = args
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| ["input", "patch", "arg"].iter().map(|k| s(k)).find(|x| !x.is_empty()).unwrap_or_default());
            p.lines().filter(|l| !l.starts_with("*** ")).map(str::to_string).collect()
        }
    }
}

/// One line for the fold and main's feed.
fn summary_of(call: &Call) -> String {
    match call.tool.as_str() {
        "bash" => crate::util::clip(&crate::util::one_line(&bash_cmd(&call.args)), 120),
        t if rules::EDIT_TOOLS.contains(&t) => format!("{} {}", t, approvals::edit_paths(t, &call.args).join(" ")),
        t => t.to_string(),
    }
}

/// The rule "always" saves for this card, as the rules file holds it.
fn rule_of(tool: &str, repo: &Path, text: &str) -> rules::Rule {
    let mut r = rules::Rule {
        project: Some(repo.to_path_buf()),
        tool: tool.to_string(),
        added: Some(crate::util::now_ms().to_string()),
        from: Some("card".into()),
        ..Default::default()
    };
    if tool == "bash" {
        r.pattern = Some(text.to_string());
    } else if rules::EDIT_TOOLS.contains(&tool) {
        r.path = Some(PathBuf::from(text));
    }
    r
}

/// The git common root of `dir` (its repo), or `dir`.
fn repo_of(dir: &str) -> PathBuf {
    std::process::Command::new("git")
        .args(["-C", dir, "rev-parse", "--path-format=absolute", "--git-common-dir"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()))
        .map(|g| if g.ends_with(".git") { g.parent().map(Path::to_path_buf).unwrap_or(g) } else { g })
        .unwrap_or_else(|| PathBuf::from(dir))
}

impl Shell {
    /// Where bise's home is (`~/.bise`, `BISE_HOME`).
    fn bise_root(&self) -> PathBuf {
        bise_home::Home::from_env().root().to_path_buf()
    }

    /// The mode file of one agent, written at its spawn and at a switch.
    pub(super) fn write_mode_file(&self, dir: &str) {
        let run = self.opts.paths.agent_run(dir);
        let _ = std::fs::create_dir_all(&run);
        let tmp = run.join(format!("{}.tmp", mode::MODE_FILE));
        if std::fs::write(&tmp, format!("{}\n", self.gates.mode.word())).is_ok() {
            let _ = std::fs::rename(&tmp, run.join(mode::MODE_FILE));
        }
    }

    /// `shift+tab`, `/approvals yolo|auto`: the new mode for every agent's
    /// next gated call; remembered in config.toml unless `BISE_APPROVALS`
    /// set it. The cards already open stay (design §1.9).
    pub(super) fn set_mode(&mut self, m: Mode) {
        self.gates.mode = m;
        if !self.gates.env {
            let path = bise_home::Home::from_env().config_file();
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let new = bise_catalog::with_key(&text, "approvals", &bise_catalog::toml_string(m.word()));
            if new != text {
                if let Some(d) = path.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                if let Err(e) = std::fs::write(&path, new) {
                    log_line(&self.opts.paths, &format!("approvals: cannot write {}: {}", path.display(), e));
                }
            }
        }
        let dirs: Vec<String> = self.hub.st.agents.values().map(|a| a.dir.clone()).collect();
        for d in dirs {
            self.write_mode_file(&d);
        }
    }

    /// The `approvals` event for the TUIs (`flash`: a switch just happened).
    pub(super) fn approvals_ev(&self, flash: bool) -> Value {
        let mut v = self.gates.info(&self.bise_root());
        v["ev"] = json!("approvals");
        v["flash"] = json!(flash);
        v
    }

    /// Append a verdict to the gate file of `dir`'s waiting call and
    /// forget it; the runtime goes on (its `gate-done` follows).
    fn resolve(&mut self, dir: &str, allow: bool, reason: &str) {
        let Some(w) = self.gates.waiting.remove(dir) else { return };
        let line = verdict_line(&w.n, &w.nonce, allow, reason);
        use std::io::Write;
        let ok = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&w.file)
            .and_then(|mut f| f.write_all(line.as_bytes()));
        if let Err(e) = ok {
            log_line(&self.opts.paths, &format!("{}: gate {}: cannot write {}: {}", w.agent, w.n, w.file.display(), e));
        }
        if w.card.is_some() {
            self.hub.set_on_you(&w.agent, false);
            let snap = self.snapshot();
            self.broadcast(&snap);
        }
    }

    /// A `gate <n> <json>` line of `dir`'s REPL.
    pub(super) fn on_gate(&mut self, dir: &str, name: &str, rest: &str) {
        let Some((n, v)) = parse_gate(rest) else {
            log_line(&self.opts.paths, &format!("{name}: unreadable gate line: {}", crate::util::clip(rest, 200)));
            return;
        };
        let nonce = v.get("nonce").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let Some(port) = self.ports.get(dir).copied() else { return };
        let file = mode::gate_file(&self.opts.paths.agent_run(dir), port);
        self.gates.waiting.insert(dir.to_string(), Waiting { n: n.clone(), nonce, file, agent: name.to_string(), card: None });
        // the runtime read a mode file older than a switch: yolo asks nothing
        if self.gates.mode == Mode::Yolo {
            return self.resolve(dir, true, "");
        }
        let tool = v.get("tool").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let args = args_of(&tool, v.get("args"));
        let call = self.call_of(dir, name, tool, args);
        if self.gates.runner.sync() {
            for c in self.gates.caches.values_mut() {
                c.clear();
            }
        }
        let rules = self.rules_now();
        let cache = self.gates.caches.entry(call.repo.clone()).or_default();
        let verdict = approvals::judge(&call, &rules, cache, false);
        self.on_verdict(dir, call, verdict);
    }

    fn on_verdict(&mut self, dir: &str, call: Call, verdict: Verdict) {
        match verdict {
            Verdict::Allow { .. } => self.resolve(dir, true, ""),
            Verdict::DenyOnce { hint, exact } => {
                let cache = self.gates.caches.entry(call.repo.clone()).or_default();
                for e in &exact {
                    cache.note_denied_once(e);
                }
                self.resolve(dir, false, &hint);
            }
            Verdict::Card { reason, always } => self.open_gate_card(dir, &call, &[], &reason, always, vec![]),
            Verdict::Check { parts, keys } => {
                if self.gates.runner.checker() == Checker::Off {
                    let tool = (parts.is_empty()).then(|| call.tool.clone());
                    let (reason, always) = match approvals::card_when_off(&parts, tool) {
                        Verdict::Card { reason, always } => (reason, always),
                        _ => (approvals::CHECKER_OFF.to_string(), None),
                    };
                    return self.open_gate_card(dir, &call, &parts, &reason, always, keys);
                }
                let Some(w) = self.gates.waiting.get(dir) else { return };
                let (agent, n) = (w.agent.clone(), w.n.clone());
                self.feed(&agent, &gate_line("check", &n, None));
                let task = self
                    .hub
                    .st
                    .agents
                    .get(&agent)
                    .map(|a| a.brief.objective.clone())
                    .unwrap_or_default();
                let req = CheckReq { call, parts, keys, task, script: None, denied: None };
                let (runner, tx, dir) = (self.gates.runner.clone(), self.tx.clone(), dir.to_string());
                std::thread::spawn(move || {
                    let out = runner.check(&req);
                    let _ = tx.send(Msg::GateChecked { dir, n, req: Box::new(req), out });
                });
            }
        }
    }

    /// The checker answered (its thread).
    pub(super) fn on_checked(&mut self, dir: &str, n: &str, req: CheckReq, out: CheckOut) {
        if let Some(line) = self.gates.runner.take_notice() {
            let _ = self.tx.send(Msg::Notice { kind: "approvals".into(), text: line });
        }
        if self.gates.waiting.get(dir).is_none_or(|w| w.n != n) {
            return; // interrupted meanwhile
        }
        match out {
            CheckOut::Allow { cache } => {
                let c = self.gates.caches.entry(req.call.repo.clone()).or_default();
                for k in cache {
                    c.allow(k);
                }
                self.resolve(dir, true, "");
            }
            CheckOut::Card { reason, detail } => {
                log_line(&self.opts.paths, &format!("approvals: checker card for {}: {}", req.call.agent, detail));
                let always = Some(approvals::always_rules(&req.parts)).filter(|a| !a.is_empty()).or_else(|| {
                    req.parts.is_empty().then(|| vec![req.call.tool.clone()]).filter(|_| req.call.tool != "bash")
                });
                self.open_gate_card(dir, &req.call, &req.parts, &reason, always, req.keys);
            }
        }
    }

    /// The call of a gate line, its folders canonical.
    fn call_of(&mut self, dir: &str, name: &str, tool: String, args: Value) -> Call {
        let cwd = self.hub.st.agents.get(name).map(|a| a.ws.path.clone()).unwrap_or_else(|| self.hub.workspace.clone());
        let repo = self.gates.repos.entry(dir.to_string()).or_insert_with(|| repo_of(&cwd)).clone();
        let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("/"));
        // the agent's edit toolset (approvals-edit: one per provider)
        let main = self.hub.st.agents.get(name).is_some_and(|a| a.is_main);
        let edit_tool = if self.in_use(dir, main).model.provider == "openai" { "apply_patch" } else { "edit" };
        Call {
            edit_tool: edit_tool.into(),
            tool,
            args,
            agent: name.to_string(),
            cwd: PathBuf::from(&cwd),
            repo,
            tmp: self.opts.paths.agent_tmp(dir),
            home,
            bise: self.bise_root(),
        }
        .canonical(&approvals::RealFs)
    }

    /// The saved rules, read again when the file changed.
    fn rules_now(&mut self) -> rules::Rules {
        let path = rules::file(&self.bise_root());
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if self.gates.rules.as_ref().is_none_or(|(t, _)| *t != mtime) {
            let r = rules::load(&path).unwrap_or_else(|e| {
                log_line(&self.opts.paths, &format!("approvals: {e}"));
                rules::Rules::default()
            });
            self.gates.rules = Some((mtime, r));
        }
        self.gates.rules.as_ref().map(|(_, r)| r.clone()).unwrap_or_default()
    }

    /// A card for `dir`'s call, or the open card of an identical call.
    fn open_gate_card(
        &mut self,
        dir: &str,
        call: &Call,
        needs: &[Part],
        reason: &str,
        always: Option<Vec<String>>,
        keys: Vec<CacheKey>,
    ) {
        let same = format!("{}\u{0}{}\u{0}{}", call.tool, call.args, call.repo.display());
        let Some(w) = self.gates.waiting.get(dir).cloned() else { return };
        let id = match self.gates.cards.iter().find(|(_, c)| c.same == same).map(|(id, _)| *id) {
            Some(id) => {
                if let Some(c) = self.gates.cards.get_mut(&id) {
                    c.dirs.push(dir.to_string());
                }
                id
            }
            None => {
                let text = card_text(call, needs, reason, always.as_deref());
                let before: Vec<u64> = self.hub.st.open_cards().map(|c| c.id).collect();
                self.step(Input::ConfirmOpen { agent: w.agent.clone(), text });
                let Some(id) = self
                    .hub
                    .st
                    .open_cards()
                    .filter(|c| c.kind == "confirm" && !before.contains(&c.id))
                    .map(|c| c.id)
                    .max()
                else {
                    return self.resolve(dir, false, "i couldn't ask the user, so this did not run.");
                };
                self.gates.cards.insert(
                    id,
                    GateCard {
                        same,
                        dirs: vec![dir.to_string()],
                        always,
                        keys,
                        tool: call.tool.clone(),
                        repo: call.repo.clone(),
                        summary: summary_of(call),
                    },
                );
                id
            }
        };
        if let Some(w) = self.gates.waiting.get_mut(dir) {
            w.card = Some(id);
        }
        self.hub.set_on_you(&w.agent, true);
        self.feed(&w.agent, &gate_line("card", &w.n, Some(id)));
        let snap = self.snapshot();
        self.broadcast(&snap);
    }

    /// The user answered card `id` (the `confirm` effect of sb-core, which
    /// closed it).
    pub(super) fn on_confirm(&mut self, id: u64, text: &str) {
        let Some(c) = self.gates.cards.remove(&id) else { return };
        let answer = answer_of(text);
        let names: Vec<String> =
            c.dirs.iter().filter_map(|d| self.gates.waiting.get(d).map(|w| w.agent.clone())).collect();
        let (allow, note) = match &answer {
            Answer::Allow => (true, String::new()),
            Answer::Always => {
                let bise = self.bise_root();
                for r in c.always.iter().flatten() {
                    if let Err(e) = rules::save(&rules::file(&bise), &rule_of(&c.tool, &c.repo, r)) {
                        log_line(&self.opts.paths, &format!("approvals: {e}"));
                    }
                }
                (true, String::new())
            }
            Answer::No(note) => {
                let cache = self.gates.caches.entry(c.repo.clone()).or_default();
                for k in &c.keys {
                    cache.forget(k);
                }
                (false, note.clone())
            }
        };
        for d in &c.dirs {
            self.resolve(d, allow, &no_reason(&note));
        }
        let fold = fold_line(allow, &names, &c.summary, &note);
        let mut feeds: Vec<String> = names.clone();
        if !feeds.iter().any(|n| n == MAIN) {
            feeds.push(MAIN.to_string());
        }
        for f in feeds {
            self.feed(&f, &fold);
        }
        self.broadcast(&self.approvals_ev(false));
    }

    /// `gate-done <n> <how>`: the runtime went on (a verdict, or an
    /// interrupt: then its card closes when no call waits on it).
    pub(super) fn on_gate_done(&mut self, dir: &str, name: &str, rest: &str) {
        let n = rest.split_whitespace().next().unwrap_or("").to_string();
        let how = rest.split_whitespace().nth(1).unwrap_or("").to_string();
        let w = self.gates.waiting.get(dir).filter(|w| w.n == n).cloned();
        if let Some(w) = w {
            self.gates.waiting.remove(dir);
            if let Some(id) = w.card {
                self.hub.set_on_you(name, false);
                let empty = self.gates.cards.get_mut(&id).map(|c| {
                    c.dirs.retain(|d| d != dir);
                    c.dirs.is_empty()
                });
                if empty == Some(true) {
                    self.gates.cards.remove(&id);
                    self.step(Input::ConfirmClose { card: id, res: "interrupted".into() });
                }
                let snap = self.snapshot();
                self.broadcast(&snap);
            }
        }
        let _ = how;
        self.feed(name, &gate_line("done", &n, None));
    }

    /// The snapshot's gate cards held by several agents: `agent` lists
    /// them (`t10a, t10b`: the TUI's "2 agents want to run").
    pub(super) fn gate_card_agents(&self, snap: &mut Value) {
        for c in snap["cards"].as_array_mut().into_iter().flatten() {
            let Some(g) = c["id"].as_u64().and_then(|id| self.gates.cards.get(&id)) else { continue };
            let names: Vec<String> = g.dirs.iter().filter_map(|d| self.gates.waiting.get(d).map(|w| w.agent.clone())).collect();
            if names.len() > 1 {
                c["agent"] = json!(names.join(", "));
            }
        }
    }

    /// After a step: a gate card the user closed without answering
    /// (`/close`, ctrl+x) is a no.
    pub(super) fn gate_sweep(&mut self) {
        let open: Vec<u64> = self.hub.st.open_cards().map(|c| c.id).collect();
        let gone: Vec<u64> = self.gates.cards.keys().filter(|id| !open.contains(id)).copied().collect();
        for id in gone {
            self.on_confirm(id, "no");
        }
    }

    /// An agent's REPL is gone: its wait ends with it.
    pub(super) fn gate_forget(&mut self, dir: &str) {
        if let Some(w) = self.gates.waiting.remove(dir) {
            self.hub.set_on_you(&w.agent, false);
            if let Some(id) = w.card {
                let empty = self.gates.cards.get_mut(&id).map(|c| {
                    c.dirs.retain(|d| d != dir);
                    c.dirs.is_empty()
                });
                if empty == Some(true) {
                    self.gates.cards.remove(&id);
                    self.step(Input::ConfirmClose { card: id, res: "interrupted".into() });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(tool: &str, args: Value) -> Call {
        Call {
            tool: tool.into(),
            args,
            agent: "api-v2".into(),
            cwd: "/r".into(),
            repo: "/r".into(),
            tmp: "/h/agents/a/tmp".into(),
            home: "/u".into(),
            bise: "/u/.bise".into(),
            edit_tool: "edit".into(),
        }
    }

    #[test]
    fn the_wire_lines() {
        let (n, v) = parse_gate(r#"7 {"tool":"bash","args":"{\"arg\":\"ls\"}","nonce":"x1"}"#).unwrap();
        assert_eq!((n.as_str(), v["nonce"].as_str()), ("7", Some("x1")));
        assert!(parse_gate("7 not json").is_none());
        assert_eq!(args_of("bash", Some(&json!("ls -la"))), json!({"arg": "ls -la"}));
        assert_eq!(args_of("bash", Some(&json!("{\"arg\":\"ls\"}"))), json!({"arg": "ls"}));
        assert_eq!(args_of("bash", Some(&json!("\"ls\""))), json!({"arg": "ls"}));
        assert_eq!(args_of("gmail.send", Some(&json!("{\"to\":1}"))), json!({"to": 1}));
        assert_eq!(verdict_line("7", "x1", true, ""), "7 x1 allow\n");
        assert_eq!(verdict_line("7", "x1", false, "the user\nsaid no."), "7 x1 deny the user said no.\n");
        assert_eq!(gate_line("card", "7", Some(12)), "sb gate : card 7 12");
        assert_eq!(
            fold_line(false, &["api-v2".into()], "git push : x", "use a branch"),
            "sb approval : no : api-v2 : git push \\: x : use a branch"
        );
    }

    #[test]
    fn what_the_answer_says() {
        assert_eq!(answer_of("allow"), Answer::Allow);
        assert_eq!(answer_of("2"), Answer::Always);
        assert_eq!(answer_of("always allow npm run build * here"), Answer::Always);
        assert_eq!(answer_of("deny: not now"), Answer::No("not now".into()));
        assert_eq!(answer_of("no"), Answer::No(String::new()));
        assert_eq!(answer_of("no: use a branch"), Answer::No("use a branch".into()));
        assert_eq!(answer_of("use a branch"), Answer::No("use a branch".into()));
        assert_eq!(no_reason("use a branch"), "the user said no: use a branch");
    }

    #[test]
    fn the_card_says_what_needs_you() {
        let c = call("bash", json!({"arg": "sb report done x && git push origin main --force"}));
        let t = card_text(&c, &[], "it rewrites main. this one always asks.", None);
        assert_eq!(
            t,
            "wants to run\n| sb report done x && git push origin main --force\nreason: it rewrites main. this one always asks."
        );
        let parts = approvals::parse::parse("ls && npm run build").parts;
        let need: Vec<Part> = parts.into_iter().filter(|p| p.text().starts_with("npm")).collect();
        let c = call("bash", json!({"arg": "ls && npm run build"}));
        let t = card_text(&c, &need, "auto: commands ask first.", Some(&["npm run build *".to_string()]));
        assert_eq!(t, "wants to run\n= ls\n| npm run build\nreason: auto: commands ask first.\nalways: npm run build *");
        let c = call("gmail.send_email", json!({"to": "a@b"}));
        assert!(card_text(&c, &[], "r", Some(&["gmail.send_email".into()])).starts_with("wants to call gmail.send_email\n| {"));
        let c = call("write_file", json!({"file_path": "/u/Desktop/x.txt", "content": "hi\nthere"}));
        assert_eq!(
            card_text(&c, &[], "r", None),
            "wants to edit a file outside the repo\n| /u/Desktop/x.txt\n| +hi\n| +there\nreason: r"
        );
    }
}
