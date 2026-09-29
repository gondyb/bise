//! Never lose what the user types (BISE-120a).
//!
//! The composer drafts (one per agent view), the image attachments they
//! name and the sent prompts (Up/Down, the newest 50) go to one file per
//! workspace, `drafts/<folder>-<hash>.json` in bise's state
//! (`bise_home::Home::drafts_dir`). It is written:
//! - [`DEBOUNCE`] after the last change, from the UI loop ([`tick`]):
//!   never on each keystroke of a burst;
//! - at once when a prompt is sent ([`save_now`]): the sent draft is gone
//!   from the file, the prompt is in the history;
//! - when the UI ends ([`flush`]): /quit, a version switch (re-exec).
//!
//! A crash or a killed terminal loses at most the last [`DEBOUNCE`].
//! [`restore`] reads it back at start. Writes are atomic (a temp file,
//! then a rename) and private (0600, the folder 0700); an empty state
//! removes the file. At start the files of workspaces that no longer
//! exist go (the tests' throwaway workspaces).
//!
//! Under `cargo test` nothing is on disk unless a test gives a folder
//! ([`use_dir`]).

use super::{App, View};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The sent prompts kept, per workspace.
pub(crate) const HISTORY_MAX: usize = 50;
/// A change is written when it has not moved for this long.
pub(crate) const DEBOUNCE: Duration = Duration::from_millis(300);

/// One draft: its text and cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Draft {
    text: String,
    cursor: usize,
}

/// What goes on disk for one workspace.
#[derive(Debug, Clone, PartialEq, Default)]
struct Saved {
    /// agent → its draft (the non-empty ones)
    drafts: BTreeMap<String, Draft>,
    /// the attachments a draft names (label, marker, what the strip says)
    attachments: Vec<crate::attach::Attachment>,
    /// newest first
    history: Vec<String>,
}

impl Saved {
    fn is_empty(&self) -> bool {
        self.drafts.is_empty() && self.attachments.is_empty() && self.history.is_empty()
    }
}

/// The saver's state (UI thread only).
#[derive(Default)]
struct State {
    /// this workspace's file (none: nothing is saved)
    file: Option<PathBuf>,
    /// the state on disk
    written: Option<Saved>,
    /// the state seen at the last tick, and since when
    seen: Option<(Saved, Instant)>,
    /// a write failed (said once in the feed)
    failed: bool,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    #[cfg(test)]
    static DIR: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// Tests: drafts on disk in `d` (per thread).
#[cfg(test)]
pub(crate) fn use_dir(d: Option<PathBuf>) {
    DIR.with(|s| *s.borrow_mut() = d);
    STATE.with(|s| *s.borrow_mut() = State::default());
}

/// The drafts folder: `bise_home::Home::drafts_dir` (`~/.bise/drafts`, or
/// the old `~/.local/state/switchboard/drafts`).
fn dir() -> Option<PathBuf> {
    #[cfg(test)]
    {
        DIR.with(|s| s.borrow().clone())
    }
    #[cfg(not(test))]
    {
        Some(bise_home::Home::from_env().drafts_dir())
    }
}

/// FNV-1a, 64 bits: a name that stays the same across Rust versions.
fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3))
}

/// `<folder name>-<hash of the path>.json`.
pub(crate) fn file_name(workspace: &str) -> String {
    let base = Path::new(workspace).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let slug: String = base
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .take(32)
        .collect();
    format!("{}-{:016x}.json", if slug.is_empty() { "ws" } else { &slug }, fnv(workspace))
}

// ---- the file ----

fn to_json(workspace: &str, s: &Saved) -> Value {
    let drafts: serde_json::Map<String, Value> = s
        .drafts
        .iter()
        .map(|(a, d)| (a.clone(), json!({"text": d.text, "cursor": d.cursor})))
        .collect();
    let atts: Vec<Value> = s
        .attachments
        .iter()
        .map(|a| {
            json!({"label": a.label, "marker": a.marker, "source": a.info.source,
                   "width": a.info.width, "height": a.info.height,
                   "bytes": a.info.bytes, "resized": a.info.resized})
        })
        .collect();
    json!({"workspace": workspace, "drafts": drafts, "attachments": atts, "history": s.history})
}

fn from_json(v: &Value) -> Saved {
    let str_of = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let num = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
    let drafts = v
        .get("drafts")
        .and_then(|d| d.as_object())
        .map(|m| {
            m.iter()
                .map(|(a, d)| (a.clone(), Draft { text: str_of(d, "text"), cursor: num(d, "cursor") as usize }))
                .filter(|(_, d)| !d.text.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let attachments = v
        .get("attachments")
        .and_then(|a| a.as_array())
        .map(|l| {
            l.iter()
                .map(|a| crate::attach::Attachment {
                    label: str_of(a, "label"),
                    marker: str_of(a, "marker"),
                    info: crate::attach::Info {
                        source: str_of(a, "source"),
                        width: num(a, "width") as u32,
                        height: num(a, "height") as u32,
                        bytes: num(a, "bytes"),
                        resized: a.get("resized").and_then(|x| x.as_bool()).unwrap_or(false),
                    },
                })
                .collect()
        })
        .unwrap_or_default();
    let mut history: Vec<String> = v
        .get("history")
        .and_then(|h| h.as_array())
        .map(|l| l.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    history.truncate(HISTORY_MAX);
    Saved { drafts, attachments, history }
}

/// Write `text` to `path` atomically, mode 0600 (the folder 0700).
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    if let Some(d) = path.parent() {
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(d)?;
    }
    let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    let r = (|| {
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r
}

fn write(st: &mut State, file: &Path, workspace: &str, s: &Saved) {
    let r = if s.is_empty() {
        match std::fs::remove_file(file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    } else {
        write_private(file, &(serde_json::to_string_pretty(&to_json(workspace, s)).unwrap_or_default() + "\n"))
    };
    st.written = Some(s.clone());
    if let Err(e) = r {
        if std::mem::replace(&mut st.failed, true) {
            return;
        }
        crate::crash::note(format!("the draft could not be saved: {}: {e}", file.display()));
    }
}

/// Remove the files of workspaces that no longer exist (not `keep`).
fn prune(dir: &Path, keep: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p == keep || p.extension().is_none_or(|x| x != "json") {
            continue;
        }
        let ws = std::fs::read_to_string(&p)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.get("workspace").and_then(|w| w.as_str()).map(PathBuf::from));
        if ws.is_some_and(|w| !w.exists()) {
            let _ = std::fs::remove_file(&p);
        }
    }
}

// ---- the app's side ----

/// What the app holds now.
fn snapshot(app: &App) -> Saved {
    let own = |ed: &crate::editor::Editor| {
        let (t, c) = ed.own_draft();
        Draft { text: t.to_string(), cursor: c }
    };
    let mut drafts = BTreeMap::new();
    let focused = own(&app.ed);
    if !focused.text.is_empty() {
        drafts.insert(app.sb.focus.clone(), focused);
    }
    for (a, v) in &app.sb.views {
        let d = own(&v.ed);
        if !d.text.is_empty() {
            drafts.insert(a.clone(), d);
        }
    }
    let attachments = app
        .attachments
        .iter()
        .filter(|a| drafts.values().any(|d: &Draft| d.text.contains(&a.label)))
        .cloned()
        .collect();
    let mut history = app.history.clone();
    history.truncate(HISTORY_MAX);
    Saved { drafts, attachments, history }
}

/// An attachment still usable: the image store still has its file.
fn still_there(a: &crate::attach::Attachment) -> bool {
    bend_images::markers(&a.marker).first().is_some_and(|m| Path::new(&m.b64).is_file())
}

/// At start: the workspace's drafts and history back in the app, the
/// saver armed.
pub(crate) fn restore(app: &mut App) {
    let Some(dir) = dir() else { return };
    let workspace = app.sb.workspace.clone();
    let file = dir.join(file_name(&workspace));
    let saved = std::fs::read_to_string(&file)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .map(|v| from_json(&v))
        .unwrap_or_default();
    {
        let (d, keep) = (dir.clone(), file.clone());
        let _ = std::thread::Builder::new().name("drafts-prune".into()).spawn(move || prune(&d, &keep));
    }
    let mut s = saved.clone();
    for (agent, d) in std::mem::take(&mut s.drafts) {
        let mut ed = crate::editor::Editor::default();
        ed.cursor = d.cursor.min(d.text.chars().count());
        ed.text = d.text;
        if agent == app.sb.focus {
            app.ed = ed;
        } else {
            app.sb.views.entry(agent).or_insert_with(View::new).ed = ed;
        }
    }
    for a in s.attachments {
        if still_there(&a) && !app.attachments.iter().any(|b| b.label == a.label) {
            app.attachments.push(a);
        }
    }
    if app.history.is_empty() {
        app.history = s.history;
    }
    STATE.with(|st| *st.borrow_mut() = State { file: Some(file), written: Some(saved), seen: None, failed: false });
}

/// The UI loop, once per turn: a state that has not moved for
/// [`DEBOUNCE`] is written.
pub(crate) fn tick(app: &App) {
    tick_at(app, Instant::now());
}

pub(crate) fn tick_at(app: &App, now: Instant) {
    STATE.with(|st| {
        let st = &mut *st.borrow_mut();
        let Some(file) = st.file.clone() else { return };
        let snap = snapshot(app);
        if st.seen.as_ref().is_none_or(|(s, _)| *s != snap) {
            st.seen = Some((snap, now));
        }
        let Some((s, since)) = &st.seen else { return };
        if st.written.as_ref() != Some(s) && now.duration_since(*since) >= DEBOUNCE {
            let s = s.clone();
            write(st, &file, &app.sb.workspace, &s);
        }
    })
}

/// Write now when the state changed (a prompt sent, the UI ends).
pub(crate) fn save_now(app: &App) {
    STATE.with(|st| {
        let st = &mut *st.borrow_mut();
        let Some(file) = st.file.clone() else { return };
        let snap = snapshot(app);
        if st.written.as_ref() != Some(&snap) {
            write(st, &file, &app.sb.workspace, &snap);
        }
        st.seen = Some((snap, Instant::now()));
    })
}

/// The UI ends: what is not written yet goes on disk.
pub(crate) fn flush(app: &App) {
    save_now(app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sb::{bench::test_app, focus, handle_input};

    /// A throwaway folder, removed at the end of the test.
    struct Tmp(PathBuf);
    impl Tmp {
        fn new() -> Tmp {
            let d = std::env::temp_dir().join(format!("bise-drafts-{}-{:?}", std::process::id(), std::thread::current().id()));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            Tmp(d)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn setup() -> (Tmp, App) {
        let d = Tmp::new();
        use_dir(Some(d.path().join("drafts")));
        let mut app = test_app();
        app.sb.workspace = d.path().join("ws").to_string_lossy().to_string();
        std::fs::create_dir_all(&app.sb.workspace).unwrap();
        restore(&mut app);
        (d, app)
    }

    fn on_disk(app: &App) -> Value {
        let f = dir().unwrap().join(file_name(&app.sb.workspace));
        serde_json::from_str(&std::fs::read_to_string(f).unwrap_or_else(|_| "{}".into())).unwrap()
    }

    /// A fresh app on the same workspace, restored (a restart).
    fn restarted(app: &App) -> App {
        let mut b = test_app();
        b.sb.workspace = app.sb.workspace.clone();
        restore(&mut b);
        b
    }

    fn typed(app: &mut App, s: &str) {
        app.ed.insert(s);
    }

    #[test]
    fn a_draft_is_written_after_the_debounce_only() {
        let (_d, mut app) = setup();
        let t0 = Instant::now();
        typed(&mut app, "half a th");
        tick_at(&app, t0);
        typed(&mut app, "ought");
        tick_at(&app, t0 + Duration::from_millis(100));
        tick_at(&app, t0 + Duration::from_millis(350));
        assert_eq!(on_disk(&app), json!({}), "the burst is still moving");
        tick_at(&app, t0 + Duration::from_millis(400));
        assert_eq!(on_disk(&app)["drafts"]["main"]["text"], "half a thought");
    }

    #[test]
    fn every_agents_draft_survives_a_restart() {
        let (_d, mut app) = setup();
        typed(&mut app, "for main");
        focus(&mut app, "docs");
        typed(&mut app, "for docs");
        flush(&app);
        let mut b = restarted(&app);
        assert_eq!(b.ed.text, "for main");
        assert_eq!(b.ed.cursor, 8);
        focus(&mut b, "docs");
        assert_eq!(b.ed.text, "for docs");
    }

    #[test]
    fn a_sent_prompt_leaves_the_draft_and_joins_the_history() {
        let (_d, mut app) = setup();
        typed(&mut app, "ship it");
        flush(&app);
        let v = app.ed.take();
        handle_input(&mut app, &format!("say {v}"));
        let disk = on_disk(&app);
        assert_eq!(disk["drafts"], json!({}), "written at once, no debounce");
        assert_eq!(disk["history"], json!(["ship it"]));
        let b = restarted(&app);
        assert_eq!(b.ed.text, "");
        assert_eq!(b.history, vec!["ship it".to_string()]);
    }

    #[test]
    fn the_history_keeps_the_newest_50() {
        let (_d, mut app) = setup();
        for i in 0..60 {
            handle_input(&mut app, &format!("say p{i}"));
        }
        assert_eq!(app.history.len(), HISTORY_MAX);
        let h = on_disk(&app)["history"].as_array().unwrap().clone();
        assert_eq!(h.len(), HISTORY_MAX);
        assert_eq!(h[0], "p59");
        assert_eq!(h[49], "p10");
    }

    #[test]
    fn browsing_the_history_saves_the_own_draft() {
        let (_d, mut app) = setup();
        app.history = vec!["old prompt".into()];
        typed(&mut app, "mine");
        assert!(app.ed.history_up(&app.history.clone()));
        assert_eq!(app.ed.text, "old prompt");
        flush(&app);
        assert_eq!(on_disk(&app)["drafts"]["main"]["text"], "mine");
    }

    #[test]
    fn the_file_is_private_and_goes_when_empty() {
        use std::os::unix::fs::PermissionsExt;
        let (_d, mut app) = setup();
        typed(&mut app, "secret");
        flush(&app);
        let f = dir().unwrap().join(file_name(&app.sb.workspace));
        assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(dir().unwrap()).unwrap().permissions().mode() & 0o777, 0o700);
        app.ed.clear();
        flush(&app);
        assert!(!f.exists());
        // no temp file left behind
        assert_eq!(std::fs::read_dir(dir().unwrap()).unwrap().count(), 0);
    }

    #[test]
    fn an_attachment_comes_back_while_its_image_is_stored() {
        let (d, mut app) = setup();
        let b64 = d.path().join("a.b64");
        std::fs::write(&b64, "AAAA").unwrap();
        let att = |label: &str, b64: &Path| crate::attach::Attachment {
            label: label.into(),
            marker: format!("<image name=\"{label}\" path=\"/x.png\" mime=\"image/png\" b64=\"{}\">", b64.display()),
            info: crate::attach::Info { source: "/x.png".into(), width: 2, height: 3, bytes: 4, resized: false },
        };
        app.attachments = vec![att("[Image #1]", &b64), att("[Image #2]", &d.path().join("gone.b64")), att("[Image #3]", &b64)];
        // #3 is not in the text: not saved
        typed(&mut app, "see [Image #1] and [Image #2]");
        flush(&app);
        assert_eq!(on_disk(&app)["attachments"].as_array().unwrap().len(), 2);
        let b = restarted(&app);
        assert_eq!(b.attachments, vec![att("[Image #1]", &b64)]);
    }

    #[test]
    fn files_of_gone_workspaces_are_pruned() {
        let d = Tmp::new();
        let dir = d.path().join("drafts");
        std::fs::create_dir_all(&dir).unwrap();
        let gone = dir.join(file_name("/nowhere/gone"));
        std::fs::write(&gone, json!({"workspace": "/nowhere/gone", "history": ["x"]}).to_string()).unwrap();
        let here = dir.join(file_name(&d.path().to_string_lossy()));
        std::fs::write(&here, json!({"workspace": d.path(), "history": ["x"]}).to_string()).unwrap();
        let other = dir.join("notes.txt");
        std::fs::write(&other, "x").unwrap();
        prune(&dir, &dir.join("mine.json"));
        assert!(!gone.exists());
        assert!(here.exists() && other.exists());
    }

    #[test]
    fn file_names_are_stable_and_safe() {
        assert_eq!(file_name("/Users/me/my repo"), format!("my_repo-{:016x}.json", fnv("/Users/me/my repo")));
        assert_ne!(file_name("/a/x"), file_name("/b/x"));
        assert!(file_name("/").starts_with("ws-"));
        // FNV-1a's reference value: it never changes under us
        assert_eq!(fnv("a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn off_without_a_folder() {
        use_dir(None);
        let mut app = test_app();
        restore(&mut app);
        typed(&mut app, "x");
        flush(&app);
        tick(&app);
    }
}
