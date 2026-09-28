//! Input of the single-agent screen: key, mouse and paste handlers,
//! the composer editor keys, voice keys, and the feed selection copy.

use crate::*;
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::text::Line;
use unicode_width::UnicodeWidthStr;

/// The feed position under the screen cell, from the last frame (the
/// feed starts at screen row `app.feed_y`: under the Switchboard header).
/// `clamp`: a row below the feed is its last row, at the end; a row
/// above it is none.
pub(crate) fn feed_pos(app: &App, x: u16, y: u16, clamp: bool) -> Option<feedsel::FeedPos> {
    let col = x.saturating_sub(app.feed_x) as usize;
    let y = y.checked_sub(app.feed_y)?;
    let (row, col) = if (y as usize) < app.vis_events.len() {
        (y as usize, col)
    } else if clamp && !app.vis_events.is_empty() {
        (app.vis_events.len() - 1, usize::MAX / 2)
    } else {
        return None;
    };
    Some((app.vis_events[row], *app.vis_rows.get(row)?, col))
}

/// The text of the feed selection (the rows of every event it spans).
pub(crate) fn feed_selection_text(app: &mut App) -> Option<String> {
    let sel = app.feed_sel?;
    let ((e0, r0, c0), (e1, r1, c1)) = sel.range();
    let (debug, w, tick) = (app.debug, app.area_w, app.tick);
    let mut rows: Vec<Line<'static>> = Vec::new();
    for i in e0..=e1.min(app.events.len().saturating_sub(1)) {
        ensure_rows(&app.events, &mut app.cache, i, debug, w, tick);
        let Some(er) = app.cache.get(i).and_then(|c| c.as_ref()) else { continue };
        let from = if i == e0 { r0 } else { 0 };
        let to = if i == e1 { (r1 + 1).min(er.rows.len()) } else { er.rows.len() };
        rows.extend(er.rows.get(from..to).unwrap_or(&[]).iter().cloned());
    }
    Some(feedsel::selection_text(&rows, c0, c1.saturating_add(1)))
}

/// Copies to the system clipboard and says so in the status row.
pub(crate) fn copy_text(app: &mut App, text: &str) {
    let n = text.chars().count();
    let note = if clipboard::copy(text) {
        format!("copied {} char{}", n, if n == 1 { "" } else { "s" })
    } else {
        "copy failed (no pbcopy, and the terminal refused OSC 52)".to_string()
    };
    app.flash = Some((note, std::time::Instant::now()));
}

/// Speech-to-text keys (Vibe's text_area._handle_voice_key): Ctrl+R
/// starts; while recording any key stops, Ctrl+C / Esc cancel; nothing
/// else sees those keys. `true` when the key was the voice's. `api_key`
/// finds MISTRAL_API_KEY (read only when a recording starts).
pub(crate) fn voice_key(
    app: &mut App,
    k: &crossterm::event::KeyEvent,
    api_key: impl FnOnce() -> Option<String>,
) -> bool {
    use voice::KeyAction;
    let now = std::time::Instant::now();
    match voice::key_action(app.voice.state(), app.voice.enabled, k.code, k.modifiers) {
        KeyAction::Pass => return false,
        KeyAction::Start => {
            if let Err(m) = app.voice.start(api_key(), now) {
                push_event(&mut app.events, &mut app.cache, Ev::Warn(m));
            }
        }
        KeyAction::Stop => app.voice.stop(now),
        KeyAction::Cancel => app.voice.cancel(),
        KeyAction::Swallow => {}
        KeyAction::OffHint => app.voice_note = Some((voice::OFF_HINT.into(), now)),
    }
    true
}

/// The transcription events of this tick: the text lands at the
/// composer cursor as it arrives.
pub(crate) fn pump_voice(app: &mut App) {
    let now = std::time::Instant::now();
    for out in app.voice.poll(now) {
        apply_voice(app, out, now);
    }
}

pub(crate) fn apply_voice(app: &mut App, out: voice::VoiceOutput, now: std::time::Instant) {
    match out {
        voice::VoiceOutput::Insert(t) => {
            app.ed.insert_voice(&t);
            app.popup_sel = 0;
        }
        voice::VoiceOutput::Utterance => app.ed.break_undo(),
        voice::VoiceOutput::Error(m) => {
            push_event(&mut app.events, &mut app.cache, Ev::Err(m));
        }
        voice::VoiceOutput::Notice(m) => app.voice_note = Some((m, now)),
    }
}

/// /voice: voice mode on or off, saved in ~/.bend-harness/tui.json.
pub(crate) fn toggle_voice(app: &mut App) -> Ev {
    let on = !app.voice.enabled;
    app.voice.enabled = on;
    if !on {
        app.voice.cancel();
    }
    match voice::save_voice_enabled(on) {
        Err(e) => Ev::Warn(format!(
            "{} (not saved: {})",
            if on { voice::ENABLED_MESSAGE } else { voice::DISABLED_MESSAGE },
            e
        )),
        Ok(()) => Ev::Info(if on { voice::ENABLED_MESSAGE } else { voice::DISABLED_MESSAGE }.into()),
    }
}

/// A key for the composer's editor (after the popups and the app keys):
/// moves, selection, deletes, undo/redo, typing, copy/cut; Up/Down move
/// between the visual rows, then through the history from the first and
/// last rows.
pub(crate) fn composer_key(app: &mut App, k: &crossterm::event::KeyEvent) {
    use editor::{Action, Motion};
    let Some(a) = editor::action(k) else { return };
    let w = app.composer.w.max(1);
    match a {
        Action::Up(sel) => {
            if !app.ed.row_up(w, sel) && (sel || !app.ed.history_up(&app.history)) {
                app.ed.move_cursor(Motion::TextStart, sel);
            }
        }
        Action::Down(sel) => {
            if !app.ed.row_down(w, sel) && (sel || !app.ed.history_down(&app.history)) {
                app.ed.move_cursor(Motion::TextEnd, sel);
            }
        }
        // the composer's selection, else the feed's
        Action::Copy => {
            if let Some(t) = app.ed.selected_text().or_else(|| feed_selection_text(app)) {
                copy_text(app, &t);
            }
        }
        Action::Cut => {
            if let Some(t) = app.ed.cut() {
                copy_text(app, &t);
                app.popup_sel = 0;
            }
        }
        Action::Insert(t) => {
            app.ed.insert(&t);
            app.popup_sel = 0;
            // a typed (never a pasted) `:name:` becomes its emoji
            if t == ":" {
                if let Some((text, cur)) = emoji::replace_typed(&app.ed.text, app.ed.cursor) {
                    app.ed.set(&text, cur);
                }
            }
        }
        other => {
            let edits = !matches!(other, Action::Move(..));
            app.ed.apply(&other);
            if edits {
                app.popup_sel = 0;
            }
        }
    }
}

/// A mouse event of the single-agent screen: help overlay, terminal
/// pane, then the feed (scroll, selection, section toggles) and the
/// composer (cursor, selection).
pub(crate) fn on_mouse(app: &mut App, m: &crossterm::event::MouseEvent, term_h: u16) {
    if help::mouse(app, m) {
        return;
    }
    if app.term.mouse(m, term_h) {
        return;
    }
    if sb::card_mouse(app, m) {
        return;
    }
    if sb::panel_mouse(app, m) {
        return;
    }
    match m.kind {
        MouseEventKind::ScrollUp => {
            app.follow = false;
            app.scroll -= 3;
        }
        MouseEventKind::ScrollDown => {
            if !app.follow {
                app.scroll += 3;
            }
        }
        // click the back-to-bottom bar to return to the tail;
        // click a thinking section to expand/collapse it
        // the composer: a press places the cursor (Shift
        // extends), a drag selects, a double click selects
        // the word, a triple click the whole text; the
        // release copies the selection
        MouseEventKind::Down(MouseButton::Left)
            if app.composer.hit(&app.ed.text, m.column, m.row, false).is_some() =>
        {
            let ci = app.composer.hit(&app.ed.text, m.column, m.row, false).unwrap_or(0);
            let clicks = app.mouse.press(m.column, m.row, std::time::Instant::now());
            match clicks {
                2 => {
                    let (a, b) = editor::word_at(&app.ed.text, ci);
                    app.ed.select_range(a, b);
                }
                3 => app.ed.select_all(),
                _ => app.ed.click(ci, m.modifiers.contains(KeyModifiers::SHIFT)),
            }
            app.mouse.drag = Some(DragIn::Composer);
        }
        MouseEventKind::Drag(MouseButton::Left) if app.mouse.drag == Some(DragIn::Composer) => {
            if let Some(ci) = app.composer.hit(&app.ed.text, m.column, m.row, true) {
                app.ed.click(ci, true);
            }
        }
        MouseEventKind::Up(MouseButton::Left) if app.mouse.drag == Some(DragIn::Composer) => {
            app.mouse.drag = None;
            if let Some(t) = app.ed.selected_text() {
                copy_text(app, &t);
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some(r) = app.bottom_bar_rect {
                let inside = m.column >= r.x
                    && m.column < r.x + r.width
                    && m.row >= r.y
                    && m.row < r.y + r.height;
                if inside {
                    app.follow = true;
                    app.unseen = 0;
                    return;
                }
            }
            // the feed: a press starts a selection (a double
            // click selects the word, a triple the row); the
            // release copies it, or toggles the section when
            // the mouse did not move
            let Some(pos) = feed_pos(app, m.column, m.row, false) else {
                app.feed_sel = None;
                return;
            };
            let clicks = app.mouse.press(m.column, m.row, std::time::Instant::now());
            let row_text = app
                .cache
                .get(pos.0)
                .and_then(|c| c.as_ref())
                .and_then(|c| c.rows.get(pos.1))
                .map(feedsel::line_text)
                .unwrap_or_default();
            let (a, b) = match clicks {
                2 => feedsel::word_cols(&row_text, pos.2),
                3 => (0, row_text.width().saturating_sub(1)),
                _ => (pos.2, pos.2),
            };
            app.feed_sel = Some(feedsel::FeedSel { anchor: (pos.0, pos.1, a), head: (pos.0, pos.1, b) });
            app.mouse.drag = Some(DragIn::Feed { moved: clicks > 1 });
        }
        MouseEventKind::Drag(MouseButton::Left) if matches!(app.mouse.drag, Some(DragIn::Feed { .. })) => {
            // dragging on the top row or below the feed scrolls
            if m.row <= app.feed_y {
                app.follow = false;
                app.scroll -= 1;
            } else if m.row as usize >= app.feed_y as usize + app.area_h && !app.follow {
                app.scroll += 1;
            }
            if let (Some(pos), Some(sel)) = (feed_pos(app, m.column, m.row, true), app.feed_sel.as_mut()) {
                if sel.head != pos {
                    sel.head = pos;
                    app.mouse.drag = Some(DragIn::Feed { moved: true });
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) if matches!(app.mouse.drag, Some(DragIn::Feed { .. })) => {
            let moved = matches!(app.mouse.drag, Some(DragIn::Feed { moved: true }));
            app.mouse.drag = None;
            if moved {
                if let Some(t) = feed_selection_text(app).filter(|t| !t.is_empty()) {
                    copy_text(app, &t);
                }
                return;
            }
            // a plain click: expand/collapse the section
            let Some(i) = app.feed_sel.take().map(|s| s.anchor.0) else { return };
            let toggled = match app.events.get_mut(i) {
                Some(Ev::Thinking { open, .. }) => {
                    *open = !*open;
                    true
                }
                Some(Ev::Tool(td)) if td.code.is_some() => {
                    td.expanded = !td.expanded;
                    true
                }
                _ => false,
            };
            if toggled {
                if let Some(c) = app.cache.get_mut(i) {
                    *c = None;
                }
            }
        }
        _ => {}
    }
}

/// A bracketed paste: the terminal pane, else the composer.
pub(crate) fn on_paste(app: &mut App, text: &str) {
    if app.term.paste(text) {
        return;
    }
    // normalize CRLF/CR so a terminal paste behaves like the
    // typed newline, then insert at the cursor
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    // a paste that is only image paths (a file dragged into the
    // terminal) attaches the images; an empty paste (Cmd+V on an image
    // in some terminals) tries the clipboard image
    let attached = if text.trim().is_empty() {
        Some(crate::attach::attach_clipboard(app).map(|l| vec![l]))
    } else {
        crate::attach::on_paste(app, &text)
    };
    match attached {
        Some(Ok(labels)) => flash(app, format!("attached {}", labels.join(" "))),
        Some(Err(e)) if !text.trim().is_empty() => {
            flash(app, format!("image not attached: {e}"));
            app.ed.paste(&text);
        }
        Some(Err(_)) => {}
        None => app.ed.paste(&text),
    }
    app.popup_sel = 0;
}

fn flash(app: &mut App, note: String) {
    app.flash = Some((note, std::time::Instant::now()));
}

/// A key event; true when it quits the UI.
pub(crate) fn on_key(app: &mut App, k: &crossterm::event::KeyEvent) -> bool {
    if help::on_key(app, k) {
        return false;
    }
    if term::on_key(app, k) {
        return false;
    }
    if k.kind != KeyEventKind::Press {
        return false;
    }
    if voice_key(app, k, voice::resolve_api_key) {
        return false;
    }
    if app.popup_dismissed.as_deref() != Some(app.ed.text.as_str()) {
        app.popup_dismissed = None;
    }
    let matches = popup_items(app);
    let popup_open = !matches.is_empty();
    let sel = matches.get(app.popup_sel.min(matches.len().saturating_sub(1)));
    if app.sb.is_some() && sb::key(app, k, popup_open) {
        return false;
    }
    if at_nav(app, k, sel) {
        return false;
    }
    match (k.code, k.modifiers) {
        // ctrl+c: INTERRUPT the running turn — the UI is
        // free immediately (local abort); the runtime kills
        // the turn at the next safe boundary (the blocking
        // model/tool call in flight cannot be cancelled),
        // and the dying turn's wire lines still render.
        // The SECOND press quits the CLI; at idle, one press
        // quits.
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            if app.interrupt_requested {
                return true;
            } else if app.pending {
                let ok = write_interrupt_flag(&app.info.interrupt_path);
                // the local abort: the composer frees and the
                // user can type right away; the flag clears
                // when the interrupted turn's idle arrives
                app.pending = false;
                app.interrupt_requested = true;
                push_event(
                    &mut app.events,
                    &mut app.cache,
                    Ev::Info(if ok {
                        "interrupted — the current turn stops at the next safe point · Ctrl+C again to quit".to_string()
                    } else {
                        "interrupt not written (side channel unreachable) — Ctrl+C again to quit".to_string()
                    }),
                );
            } else {
                return true;
            }
        }
        // ctrl+t: expand/collapse every thinking section
        (KeyCode::Char('t'), KeyModifiers::CONTROL) => {
            app.show_thinking = !app.show_thinking;
            for e in app.events.iter_mut() {
                if let Ev::Thinking { open, .. } = e {
                    *open = app.show_thinking;
                }
            }
            app.cache.clear();
        }
        // ctrl+l: clear the local feed
        (KeyCode::Char('l'), KeyModifiers::CONTROL) if app.sb.is_some() => {
            sb::clear_display(app);
        }
        (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
            app.events.clear();
            app.cache.clear();
            app.anchor = (0, 0);
        app.scroll = 0;
            app.follow = true;
            app.unseen = 0;
        }
        // esc: close the popup, else drop the selection — it
        // never interrupts (Ctrl+C does, through the flag
        // side-channel)
        (KeyCode::Esc, _) => {
            if sel.is_some_and(|c| c.closable) {
                // close the list, keep the text
                app.popup_dismissed = Some(app.ed.text.clone());
                app.popup_sel = 0;
            } else if popup_open {
                app.ed.clear();
            } else {
                app.ed.anchor = None;
                app.feed_sel = None;
            }
        }
        // scrollback: PgUp/PgDn page, End follows the bottom
        (KeyCode::PageUp, _) => {
            let page = (app.area_h / 2).max(1);
            app.follow = false;
            app.scroll -= page as isize;
        }
        (KeyCode::PageDown, _) => {
            let page = (app.area_h / 2).max(1);
            if !app.follow {
                app.scroll += page as isize;
            }
        }
        // End back to the tail when scrolled up, else the
        // line end (the editor)
        (KeyCode::End, KeyModifiers::NONE) if !app.follow => {
            app.follow = true;
            app.unseen = 0;
        }
        (KeyCode::Tab, _) => {
            if let Some(c) = sel {
                // popup completion
                pick(app, c);
            } else if app.pending {
                // codex queue_keys: queue the draft for after
                // the turn ("say" forces the message reading
                // even if the text starts with a protocol word)
                let v = app.ed.text.trim().to_string();
                if !v.is_empty() && !v.starts_with('/') {
                    app.ed.take();
                    let v = crate::attach::expand(app, &v);
                    handle_input(app, &format!("say {}", v));
                }
            }
        }
        // a newline in the composer: Shift+Enter (needs
        // the kitty keyboard protocol), Ctrl+J (LF, the one
        // binding EVERY terminal transmits), or alt+enter;
        // plain Enter sends
        // ctrl+v: attach the clipboard image (terminals paste text only)
        (KeyCode::Char('v'), KeyModifiers::CONTROL) => match crate::attach::attach_clipboard(app) {
            Ok(l) => flash(app, format!("attached {l}")),
            Err(e) => flash(app, e),
        },
        (KeyCode::Enter, KeyModifiers::SHIFT)
        | (KeyCode::Char('j'), KeyModifiers::CONTROL)
        | (KeyCode::Enter, KeyModifiers::ALT) => {
            app.ed.insert("\n");
        }
        (KeyCode::Enter, _) => {
            if let Some(c) = sel {
                if let Some(v) = c.run.clone() {
                    app.ed.take();
                    handle_input(app, &v);
                } else {
                    pick(app, c);
                }
            } else {
                let v = app.ed.take().trim().to_string();
                let v = if v.starts_with('/') { v } else { crate::attach::expand(app, &v) };
                app.follow = true;
                app.unseen = 0;
                if !v.is_empty() {
                    // codex semantics: while the agent works,
                    // Enter STEERS the running turn; at idle it
                    // starts one. Commands pass through. The
                    // explicit "say" keeps text that starts
                    // with a protocol word ("reload ce
                    // fichier", "compact la fonction") a
                    // message, never a command.
                    let line = if v.starts_with('/') {
                        v
                    } else if app.pending {
                        format!("steer {}", v)
                    } else {
                        format!("say {}", v)
                    };
                    handle_input(app, &line);
                }
            }
        }
        // the popup takes the plain arrows
        (KeyCode::Up, KeyModifiers::NONE) if popup_open => {
            app.popup_sel = popup_step(app.popup_sel, matches.len(), false);
        }
        (KeyCode::Down, KeyModifiers::NONE) if popup_open => {
            app.popup_sel = popup_step(app.popup_sel, matches.len(), true);
        }
        _ => composer_key(app, k),
    }
    false
}

/// The selection one row down (or up) in a popup of `len` rows,
/// wrapping; 0 when the list is empty or the selection is stale.
pub(crate) fn popup_step(sel: usize, len: usize, down: bool) -> usize {
    match len {
        0 => 0,
        _ if sel >= len => 0,
        _ if down => (sel + 1) % len,
        _ => sel.checked_sub(1).unwrap_or(len - 1),
    }
}

/// The folder keys of the `@` popup: → on a folder row browses it (the
/// popup stays open on its entries); ← or Backspace on `@dir/` goes one
/// folder up. True when the key was taken.
fn at_nav(app: &mut App, k: &crossterm::event::KeyEvent, sel: Option<&PopItem>) -> bool {
    match (k.code, k.modifiers) {
        (KeyCode::Right, KeyModifiers::NONE) => match sel {
            Some(c) if c.folder => {
                pick(app, c);
                true
            }
            _ => false,
        },
        (KeyCode::Left | KeyCode::Backspace, KeyModifiers::NONE) => match commands::at_up(app) {
            Some((text, cursor)) => {
                app.ed.set(&text, cursor);
                app.popup_sel = 0;
                true
            }
            None => false,
        },
        _ => false,
    }
}

/// Take the popup entry `c` into the composer (a picked path ranks first
/// in the next `@` searches; a folder is browsed, not picked).
fn pick(app: &mut App, c: &PopItem) {
    if let Some(p) = c.path.as_ref().filter(|_| !c.folder) {
        files::picked(p);
        // an image is attached, not inserted as a path
        match crate::attach::pick_image(app, p) {
            Some(Ok(l)) => {
                flash(app, format!("attached {l} {p}"));
                app.popup_sel = 0;
                return;
            }
            Some(Err(e)) => flash(app, format!("image not attached: {e}")),
            None => {}
        }
    }
    app.ed.set(&c.fill, c.fill_cursor);
    app.popup_sel = 0;
}
