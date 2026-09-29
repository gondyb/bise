//! Slash commands and the composer autocomplete popup (commands,
//! agents, files, skills, emoji).

use crate::app::*;
use crate::*;

// ---- slash commands (codex-style) ----

pub(crate) struct Cmd {
    pub(crate) name: &'static str,
    pub(crate) desc: &'static str,
    pub(crate) args: bool,
}

/// The slash commands the popup offers and `/help` lists.
pub(crate) const COMMANDS: &[Cmd] = &[
    Cmd {
        name: "/voice",
        desc: "turn voice mode (ctrl+r speech-to-text) on or off",
        args: false,
    },
    Cmd {
        name: "/restart",
        desc: "rebuild and restart switchboard on the latest commit (agents kept): /restart [current|<commit>]",
        args: true,
    },
    Cmd {
        name: "/version",
        desc: "switchboard versions: /version [<commit>|tree|back]",
        args: true,
    },
    Cmd {
        name: "/new",
        desc: "start an agent: /new [-w] [name:] objective",
        args: true,
    },
    Cmd {
        name: "/drop",
        desc: "stop and archive an agent (and its worktree)",
        args: true,
    },
    Cmd {
        name: "/restore",
        desc: "reopen an archived agent",
        args: true,
    },
    Cmd {
        name: "/archived",
        desc: "show or hide the archived agents in the panel",
        args: false,
    },
    Cmd {
        name: "/isolate",
        desc: "give an agent its own git worktree",
        args: true,
    },
    Cmd {
        name: "/rename",
        desc: "rename an agent",
        args: true,
    },
    Cmd {
        name: "/answer",
        desc: "answer a card: /answer N text",
        args: true,
    },
    Cmd {
        name: "/close",
        desc: "close a card without answering: /close N",
        args: true,
    },
    Cmd {
        name: "/plugins",
        desc: "the workspace's agent plugins (enable|disable name)",
        args: true,
    },
    Cmd {
        name: "/agents",
        desc: "list the agents and what they do",
        args: false,
    },
    Cmd {
        name: "/interrupt",
        desc: "interrupt the turn of the agent in view",
        args: false,
    },
    Cmd {
        name: "/compact",
        desc: "compact the conversation of the agent in view",
        args: false,
    },
    Cmd {
        name: "/theme",
        desc: "light, dark, or auto (your terminal's background): /theme [auto|light|dark]",
        args: true,
    },
    Cmd {
        name: "/welcome",
        desc: "replay the welcome of the first launch",
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
        desc: "quit (the agents keep running)",
        args: false,
    },
];

// BR-002/BR-003: the interrupt side-channel flag, shared by the Ctrl+C
// key and the /interrupt command
pub(crate) fn popup_matches(input: &str) -> Vec<&'static Cmd> {
    if !input.starts_with('/') || input.contains(' ') {
        return Vec::new();
    }
    COMMANDS.iter().filter(|c| c.name.starts_with(input)).collect()
}

/// One entry of the composer popup: a slash command, or an agent name
/// after `@`.
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
    /// a folder of the `@` popup: `fill` browses it (`@path/`, the popup
    /// stays open on its entries); Tab, Enter and → take it
    pub(crate) folder: bool,
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
                folder: false,
            })
            .collect();
    }
    let versions = sb::version_items(app);
    if !versions.is_empty() {
        return versions;
    }
    let cards = sb::close_items(app);
    if !cards.is_empty() {
        return cards;
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
const DIR_MARK: &str = theme::G_CLOSED; // a folder opens: the disclosure mark

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
            mark: Some(m.glyph(app.tick, app.motion)),
            fill,
            fill_cursor,
            run: None,
            closable: true,
            path: None,
            folder: false,
        }
    });
    // the Switchboard workspace, else the folder the TUI runs in
    let root = sb::workspace(app)
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    let mut hits = files::search(&root, &q, FILE_ROWS);
    // browsing a folder: the folder itself last (↑ from the first row),
    // so ⏎ can still insert a folder reference
    let this = files::parent_query(&q)
        .and_then(|_| hits.first())
        .and_then(|h| h.path.rsplit_once('/'))
        .map(|(parent, _)| parent)
        .filter(|parent| parent.to_lowercase() == q.trim_end_matches('/').to_lowercase())
        .map(|parent| files::Hit { path: parent.to_string(), dir: true });
    if this.is_some() {
        hits.truncate(FILE_ROWS - 1);
    }
    let files = hits.into_iter().map(|h| {
        let (fill, fill_cursor) = if h.dir {
            files::replace_token(&app.ed.text, start, app.ed.cursor, &files::browse(&h.path))
        } else {
            fill(&files::reference(&h.path, false))
        };
        PopItem {
            label: format!("{}{}", h.path, if h.dir { "/" } else { "" }),
            desc: String::new(),
            mark: Some(if h.dir { (theme::glyph(DIR_MARK), theme::accent()) } else { (FILE_MARK, theme::dim()) }),
            fill,
            fill_cursor,
            run: None,
            closable: true,
            path: Some(h.path),
            folder: h.dir,
        }
    });
    let this = this.map(|h| {
        let (fill, fill_cursor) = fill(&files::reference(&h.path, true));
        PopItem {
            label: format!("{}/", h.path),
            desc: "this folder".into(),
            mark: Some((theme::glyph(DIR_MARK), theme::accent())),
            fill,
            fill_cursor,
            run: None,
            closable: true,
            path: Some(h.path),
            folder: false,
        }
    });
    agents.chain(files).chain(this).collect()
}

/// ← or Backspace while the `@` popup browses a folder (`@rust/tui/`):
/// the composer one folder up (`@rust/`), the popup still open. None
/// when the token does not end with `/` (the key edits as usual).
pub(crate) fn at_up(app: &App) -> Option<(String, usize)> {
    if !popup_open(app) || app.ed.anchor.is_some() {
        return None;
    }
    let (start, q) = files::token(&app.ed.text, app.ed.cursor)?;
    let up = files::parent_query(&q)?;
    Some(files::replace_token(&app.ed.text, start, app.ed.cursor, &files::browse(up)))
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
                folder: false,
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
                mark: Some((e.glyph, theme::text())),
                fill,
                fill_cursor,
                run: None,
                closable: true,
                path: None,
                folder: false,
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

/// One line typed by the user: `sb::handle_input` (the hub interprets it).
pub(crate) use crate::sb::handle_input;

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
