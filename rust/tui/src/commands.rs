//! Slash commands and the composer autocomplete popup (commands,
//! skills, emoji), plus the startup harness-info of the REPL.

use crate::app::*;
use crate::feed::*;
use crate::*;

// ---- slash commands (codex-style) ----

pub(crate) struct Cmd {
    pub(crate) name: &'static str,
    pub(crate) desc: &'static str,
    pub(crate) args: bool,
}

pub(crate) const COMMANDS: &[Cmd] = &[
    Cmd {
        name: "/compact",
        desc: "compact the conversation (summary)",
        args: false,
    },
    Cmd {
        name: "/interrupt",
        desc: "interrupt the current turn",
        args: false,
    },
    Cmd {
        name: "/reload",
        desc: "restart the harness with the latest code (session kept)",
        args: false,
    },
    Cmd {
        name: "/plugins",
        desc: "the agent plugins (enable|disable NAME)",
        args: true,
    },
    Cmd {
        name: "/status",
        desc: "model, connection, compaction threshold",
        args: false,
    },
    Cmd {
        name: "/clear",
        desc: "clear the local display",
        args: false,
    },
    Cmd {
        name: "/voice",
        desc: "turn voice mode (Ctrl+R speech-to-text) on or off",
        args: false,
    },
    Cmd {
        name: "/help",
        desc: "the commands and the essential keys",
        args: false,
    },
    Cmd {
        name: "/shortcuts",
        desc: "every keyboard shortcut (also /keys)",
        args: false,
    },
    Cmd {
        name: "/quit",
        desc: "quit the client (the session survives)",
        args: false,
    },
];

// BR-002/BR-003: the interrupt side-channel flag, shared by the Ctrl+C
// key and the /interrupt command
/// What the Bend REPL announces at startup (its `harness-info` line):
/// the REPL is the single source of truth, the TUI never recomputes
/// the model, the threshold or the side-channel paths.
#[derive(Clone, Debug, Default)]
pub struct HarnessInfo {
    pub model: String,
    pub threshold: String,
    pub steer_path: String,
    pub interrupt_path: String,
}

impl HarnessInfo {
    /// Parse `harness-info model=M threshold=N steer=P interrupt=Q`.
    pub fn parse(line: &str) -> Option<HarnessInfo> {
        let rest = line.trim().strip_prefix("harness-info ")?;
        let mut info = HarnessInfo::default();
        for kv in rest.split_whitespace() {
            let (k, v) = kv.split_once('=')?;
            match k {
                "model" => info.model = v.to_string(),
                "threshold" => info.threshold = v.to_string(),
                "steer" => info.steer_path = v.to_string(),
                "interrupt" => info.interrupt_path = v.to_string(),
                _ => {}
            }
        }
        if info.model.is_empty() || info.steer_path.is_empty() || info.interrupt_path.is_empty() {
            return None;
        }
        Some(info)
    }

    /// Find the line in a REPL log.
    pub fn from_log(log: &str) -> Option<HarnessInfo> {
        log.lines().find_map(HarnessInfo::parse)
    }
}

pub(crate) fn write_interrupt_flag(path: &str) -> bool {
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .and_then(|mut f| f.write_all(b"1"))
        .is_ok()
}

pub(crate) fn popup_matches(input: &str) -> Vec<&'static Cmd> {
    if !input.starts_with('/') || input.contains(' ') {
        return Vec::new();
    }
    let list = if sb::SB_MODE.load(std::sync::atomic::Ordering::SeqCst) {
        sb::SB_COMMANDS
    } else {
        COMMANDS
    };
    list.iter().filter(|c| c.name.starts_with(input)).collect()
}

/// One entry of the composer popup: a slash command, or an agent name
/// after `@` (switchboard mode).
pub(crate) struct PopItem {
    pub(crate) label: String,
    pub(crate) desc: String,
    /// status glyph (mentions)
    pub(crate) mark: Option<(&'static str, Color)>,
    /// the composer text once picked with Tab (or Enter when `run` is
    /// None), and the cursor in it
    pub(crate) fill: String,
    pub(crate) fill_cursor: usize,
    /// Enter runs this line directly (commands without arguments)
    pub(crate) run: Option<String>,
    /// Esc closes the list and keeps the text (`@` and `$`); the slash
    /// popup clears the draft instead
    pub(crate) closable: bool,
    /// a workspace path: remembered when picked (ranked first next time)
    pub(crate) path: Option<String>,
}

pub(crate) fn popup_items(app: &App) -> Vec<PopItem> {
    let cmds = popup_matches(&app.ed.text);
    if !cmds.is_empty() {
        return cmds
            .into_iter()
            .map(|c| PopItem {
                label: c.name.to_string(),
                desc: c.desc.to_string(),
                mark: None,
                fill: format!("{} ", c.name),
                fill_cursor: c.name.chars().count() + 1,
                run: (!c.args).then(|| c.name.to_string()),
                closable: false,
                path: None,
            })
            .collect();
    }
    let versions = sb::version_items(app);
    if !versions.is_empty() {
        return versions;
    }
    let at = at_items(app);
    if !at.is_empty() {
        return at;
    }
    let skills = skill_items(app);
    if skills.is_empty() {
        emoji_items(app)
    } else {
        skills
    }
}

/// A `@`, `$`, `:` or `/version` popup may complete the draft: not while
/// a history line is recalled, nor after Esc closed the list on this text.
pub(crate) fn popup_open(app: &App) -> bool {
    !app.ed.browsing() && app.popup_dismissed.as_deref() != Some(app.ed.text.as_str())
}

/// Files listed after the agents in the `@` popup.
const FILE_ROWS: usize = 50;
/// The file and folder marks of the `@` popup.
const FILE_MARK: &str = "▪";
const DIR_MARK: &str = "▸";

/// `@word` at the start or inline: the live agents (Switchboard), then
/// the files and folders of the workspace (files.rs). A file inserts its
/// relative path, an agent `@name`.
pub(crate) fn at_items(app: &App) -> Vec<PopItem> {
    if !popup_open(app) {
        return Vec::new();
    }
    let Some((start, q)) = files::token(&app.ed.text, app.ed.cursor) else {
        return Vec::new();
    };
    let fill = |ins: &str| files::complete(&app.ed.text, start, app.ed.cursor, ins);
    let agents = sb::mentions(app, &q).into_iter().map(|m| {
        let (fill, fill_cursor) = fill(&m.completion());
        PopItem {
            label: format!("@{}", m.name),
            desc: if m.objective.is_empty() {
                m.status.clone()
            } else {
                format!("{} · {}", m.status, m.objective)
            },
            mark: Some(m.glyph(app.tick)),
            fill,
            fill_cursor,
            run: None,
            closable: true,
            path: None,
        }
    });
    // the Switchboard workspace, else the folder the TUI runs in
    let root = sb::workspace(app)
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    let files = files::search(&root, &q, FILE_ROWS).into_iter().map(|h| {
        let (fill, fill_cursor) = fill(&files::reference(&h.path, h.dir));
        PopItem {
            label: if h.dir { format!("{}/", h.path) } else { h.path.clone() },
            desc: String::new(),
            mark: Some(if h.dir { (DIR_MARK, BRAND) } else { (FILE_MARK, DIM) }),
            fill,
            fill_cursor,
            run: None,
            closable: true,
            path: Some(h.path),
        }
    });
    agents.chain(files).collect()
}

/// `$skill` anywhere in the draft: the skills of the index.
pub(crate) fn skill_items(app: &App) -> Vec<PopItem> {
    if !popup_open(app) {
        return Vec::new();
    }
    let Some((start, q)) = skills::token(&app.ed.text, app.ed.cursor) else {
        return Vec::new();
    };
    let all = skills::index();
    skills::filter(&all, &q)
        .into_iter()
        .map(|s| {
            let (fill, fill_cursor) = skills::complete(&app.ed.text, start, app.ed.cursor, &s.name);
            PopItem {
                label: format!("${}", s.name),
                desc: s.desc.clone(),
                mark: None,
                fill,
                fill_cursor,
                run: None,
                closable: true,
                path: None,
            }
        })
        .collect()
}

/// `:name` anywhere in the draft: the matching emojis (emoji.rs).
pub(crate) fn emoji_items(app: &App) -> Vec<PopItem> {
    if !popup_open(app) {
        return Vec::new();
    }
    let Some((start, q)) = emoji::token(&app.ed.text, app.ed.cursor) else {
        return Vec::new();
    };
    emoji::filter(&q)
        .into_iter()
        .map(|(e, name)| {
            let (fill, fill_cursor) = emoji::complete(&app.ed.text, start, app.ed.cursor, e.glyph);
            PopItem {
                label: format!(":{}:", name),
                desc: e.desc.to_string(),
                mark: Some((e.glyph, TEXT)),
                fill,
                fill_cursor,
                run: None,
                closable: true,
                path: None,
            }
        })
        .collect()
}

/// First visible row of a popup of `len` entries showing `rows`, so the
/// selection `sel` stays in view.
pub(crate) fn popup_top(sel: usize, len: usize, rows: usize) -> usize {
    if len <= rows {
        0
    } else {
        sel.min(len - 1).saturating_sub(rows - 1)
    }
}

// interprets one user line: slash command, raw protocol word, or plain
// message (implicit "say"). Returns the local events produced (echo
// included) so line mode can print them.
// Interprets one user line. The COMMAND LANGUAGE lives in the Bend REPL
// (core/commands.bend, pinned by LAWS.bend): plain text is an implicit
// "say", /commands map to protocol words, unknown ones get a server-side
// warning. This client only handles its own lifecycle and display.
pub(crate) fn handle_input(app: &mut App, v: &str) -> Vec<Ev> {
    if v.trim() == "/voice" {
        let ev = toggle_voice(app);
        push_event(&mut app.events, &mut app.cache, ev.clone());
        return vec![ev];
    }
    if app.sb.is_some() {
        return sb::handle_input(app, v);
    }
    // the steer/say wrappers are transport, not what the user typed
    let typed = v
        .strip_prefix("steer ")
        .or_else(|| v.strip_prefix("say "))
        .unwrap_or(v);
    let mut out = vec![Ev::You(typed.to_string())];
    app.history.insert(0, v.to_string());
    app.popup_sel = 0;

    let first = v.split_whitespace().next().unwrap_or("");
    let first = if first == "/exit" { "/quit" } else { first };

    if first == "/quit" {
        // client lifecycle: closing here, no server round-trip
        app.should_quit = true;
    } else if first == "/clear" {
        app.events.clear();
        app.cache.clear();
        app.anchor = (0, 0);
        app.scroll = 0;
        app.follow = true;
        app.unseen = 0;
        out.push(Ev::Info("display cleared".into()));
    } else if first == "/plugins" {
        let ws = crate::plugins::single_workspace();
        let rep = crate::plugins::session_report(app.port);
        out.push(Ev::Info(crate::plugins::command(v, &ws, Some(&rep))));
    } else if first == "/status" {
        out.push(Ev::Info(format!(
            "model {} · {}:{} · compaction threshold {} · session {}",
            app.info.model,
            app.host,
            app.port,
            app.info.threshold,
            app.session_id
        )));
    } else if let Some(page) = help::page_of(first) {
        app.help = Some(help::Overlay::new(page));
    } else if first == "/interrupt" {
        // BR-003: the socket is only read between turns, so sending the
        // line to the harness could never interrupt anything - the
        // command goes through the same flag file as Ctrl+C while a
        // turn runs, and says so at idle
        if app.pending {
            let ok = write_interrupt_flag(&app.info.interrupt_path);
            app.pending = false;
            app.interrupt_requested = true;
            out.push(Ev::Info(if ok {
                "interrupted — the current turn stops at the next safe point".into()
            } else {
                "interrupt not written (side channel unreachable)".into()
            }));
        } else {
            out.push(Ev::Info("no turn in progress to interrupt".into()));
        }
    } else if first == "steer" && app.pending {
        // mid-turn steering goes through the FILE side-channel: the
        // harness reads the socket only between turns, but the runtime
        // drains the announced steer file at every model/tool safe
        // boundary and commits the text into the running turn (ADR 0005)
        let msg = v.strip_prefix("steer ").map(|s| s.trim()).unwrap_or(typed);
        if msg.is_empty() {
            out.push(Ev::Info("steering vide".into()));
        } else {
            let path = app.info.steer_path.clone();
            let mut line = String::with_capacity(msg.len() + 1);
            line.push_str(msg);
            line.push('\n');
            let ok = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .and_then(|mut f| f.write_all(line.as_bytes()))
                .is_ok();
            out.push(Ev::Info(if ok {
                format!("steering queued: {}", msg)
            } else {
                "steering not written (side channel unreachable)".to_string()
            }));
        }
    } else {
        // everything else — plain text, /commands, raw protocol words —
        // goes to the harness verbatim; it interprets
        app.send(v.trim());
    }

    for ev in &out {
        push_event(&mut app.events, &mut app.cache, ev.clone());
    }
    out
}

#[cfg(test)]
mod harness_info_tests {
    use super::HarnessInfo;

    // the exact string LAWS.bend pins for Rt.info_line (law
    // info_line_format): the two sides of the contract agree
    const BEND_LINE: &str = "harness-info model=claude-opus-5-5 threshold=800000 steer=/tmp/bend-steer-7.txt interrupt=/tmp/bend-interrupt-7.txt";

    #[test]
    fn parses_the_line_bend_prints() {
        let info = HarnessInfo::parse(BEND_LINE).expect("parses");
        assert_eq!(info.model, "claude-opus-5-5");
        assert_eq!(info.threshold, "800000");
        assert_eq!(info.steer_path, "/tmp/bend-steer-7.txt");
        assert_eq!(info.interrupt_path, "/tmp/bend-interrupt-7.txt");
    }

    #[test]
    fn finds_the_line_in_a_repl_log() {
        let log = format!("{}\nbend-harness LIVE REPL on 127.0.0.1:7 ...\n[mcp] connector index written\n", BEND_LINE);
        assert_eq!(HarnessInfo::from_log(&log).expect("found").model, "claude-opus-5-5");
    }

    #[test]
    fn rejects_an_incomplete_line() {
        assert!(HarnessInfo::parse("harness-info model=m threshold=1").is_none());
        assert!(HarnessInfo::parse("bend-harness LIVE REPL on 127.0.0.1:7").is_none());
    }
}

#[cfg(test)]
mod popup_tests {
    use super::popup_top;

    #[test]
    fn the_selection_stays_in_view() {
        assert_eq!(popup_top(0, 5, 8), 0);
        assert_eq!(popup_top(4, 5, 8), 0);
        assert_eq!(popup_top(7, 12, 8), 0);
        assert_eq!(popup_top(8, 12, 8), 1);
        assert_eq!(popup_top(11, 12, 8), 4);
        assert_eq!(popup_top(99, 12, 8), 4); // clamped like the selection
    }
}
