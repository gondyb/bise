//! The keys of the switchboard mode: the panel navigation, the drop and
//! not-delivered questions, the cards, esc (moved out of sb.rs as is).

use super::*;

/// Agent navigation from the keyboard.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Nav {
    Next,
    Prev,
    /// agent number N of the list (0 = main)
    Goto(usize),
}

/// Ctrl+K / Alt+↓ next, Ctrl+J / Alt+↑ previous, Alt+N agent N (0 = main).
pub(super) fn nav_key(k: &crossterm::event::KeyEvent) -> Option<Nav> {
    match (k.code, k.modifiers) {
        (KeyCode::Char('k'), KeyModifiers::CONTROL) | (KeyCode::Down, KeyModifiers::ALT) => {
            Some(Nav::Next)
        }
        (KeyCode::Char('j'), KeyModifiers::CONTROL) | (KeyCode::Up, KeyModifiers::ALT) => {
            Some(Nav::Prev)
        }
        (KeyCode::Char(c), KeyModifiers::ALT) if c.is_ascii_digit() => {
            c.to_digit(10).map(|d| Nav::Goto(d as usize))
        }
        _ => None,
    }
}

/// Keys of the switchboard mode; `true` when handled.
pub(crate) fn key(app: &mut App, k: &crossterm::event::KeyEvent, popup_open: bool) -> bool {
    let empty = app.ed.text.is_empty();
    let pending = app.pending;
    let interrupt_requested = app.interrupt_requested;
    let sb = &mut app.sb;
    // `D` asked "drop {name}?": y drops, n or esc keeps it; any other
    // key drops the question and does its usual job
    if let Some(name) = sb.drop_ask.take() {
        let plain = !k.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
        match k.code {
            KeyCode::Char('y') | KeyCode::Char('Y') if plain => {
                sb.send_input(format!("/drop {}", name));
                return true;
            }
            KeyCode::Char('n') | KeyCode::Char('N') if plain => return true,
            KeyCode::Esc => return true,
            _ => {}
        }
    }
    // BISE-86: `✗ not delivered: {name} stopped. ⏎ send again · esc drop`
    // is the last line and the composer is empty: ⏎ sends it again, esc
    // drops it; the question goes away either way
    if empty && matches!(k.code, KeyCode::Enter | KeyCode::Esc) && k.modifiers == KeyModifiers::NONE {
        let last = app.events.iter().rposition(|e| crate::feed::ev_visible(e, false));
        if let Some(i) = last {
            if let Ev::Undelivered { name, text, open: open @ true } = &mut app.events[i] {
                *open = false;
                app.cache[i] = None;
                let (name, text) = (name.clone(), text.clone());
                if k.code == KeyCode::Enter {
                    let v = if sb.focus == name { text.clone() } else { format!("@{} {}", name, text) };
                    sb.send_input(v.clone());
                    push_event(&mut app.events, &mut app.cache, Ev::You(v, Mark::Sent));
                }
                return true;
            }
        }
    }
    let n = sb.nav().len();
    let nav = nav_key(k);
    match (k.code, k.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) if pending && !interrupt_requested => {
            let f = sb.focus.clone();
            sb.send(json!({"op": "interrupt", "agent": f}));
            app.interrupt_requested = true;
            push_event(
                &mut app.events,
                &mut app.cache,
                Ev::Info("interrupted — the turn stops at the next safe point · ctrl+c again to quit".into()),
            );
            true
        }
        _ if empty && n > 0 && nav == Some(Nav::Next) => {
            sb.selected = Some(match sb.selected {
                None => 0,
                Some(i) => (i + 1) % n,
            });
            true
        }
        _ if empty && n > 0 && nav == Some(Nav::Prev) => {
            sb.selected = Some(match sb.selected {
                None | Some(0) => n - 1,
                Some(i) => i - 1,
            });
            true
        }
        (KeyCode::Enter, KeyModifiers::NONE) if empty && sb.selected.is_some() => {
            if let Some(name) = sb.selected_agent().map(|a| a.name.clone()) {
                focus(app, &name);
            }
            true
        }
        (KeyCode::Char(' '), _) if empty && sb.selected.is_some() => {
            sb.preview = !sb.preview;
            true
        }
        (KeyCode::Char('A'), _) if empty && sb.selected.is_some() => {
            sb.toggle_archived();
            true
        }
        (KeyCode::Char('D'), _) if empty && sb.selected.is_some() => {
            let live = |a: &&Agent| !a.main && !a.archived();
            if let Some(name) = sb.selected_agent().filter(live).map(|a| a.name.clone()) {
                sb.drop_ask = Some(name);
            }
            true
        }
        (KeyCode::Esc, _) if !popup_open => {
            if sb.card.full {
                sb.card.full = false;
                return true;
            }
            if let Some((id, _)) = sb.confirm.take() {
                sb.send(json!({"op": "confirm", "id": id, "yes": false}));
                return true;
            }
            if sb.selected.is_some() || sb.preview {
                sb.selected = None;
                sb.preview = false;
                return true;
            }
            if sb.card.shown && empty {
                sb.card.shown = false;
                return true;
            }
            if !empty {
                // the draft goes to the history (Up brings it back)
                let d = app.ed.take();
                app.history.insert(0, d);
                return true;
            }
            if sb.focus != "main" {
                focus(app, "main");
                return true;
            }
            false
        }
        _ if matches!(nav, Some(Nav::Goto(_))) => {
            // the number shown in the panel (it stays while the agent lives)
            if let Some(t) = nav.and_then(|n| if let Nav::Goto(i) = n { sb.agent_numbered(i) } else { None }) {
                focus(app, &t);
            }
            true
        }
        (KeyCode::Char('g'), KeyModifiers::CONTROL) if !sb.cards.is_empty() => {
            sb.toggle_card();
            true
        }
        // on an empty composer, Ctrl+A (line start) has nothing to do
        (KeyCode::Char('a'), KeyModifiers::CONTROL) if empty && !sb.cards.is_empty() => {
            sb.toggle_card();
            true
        }
        (KeyCode::Char('f'), KeyModifiers::CONTROL) if sb.card.shown => {
            sb.card.full = !sb.card.full;
            true
        }
        (KeyCode::Char('n'), KeyModifiers::CONTROL) if !sb.cards.is_empty() => {
            sb.step_card(1);
            true
        }
        (KeyCode::Char('p'), KeyModifiers::CONTROL) if !sb.cards.is_empty() => {
            sb.step_card(-1);
            true
        }
        (KeyCode::PageUp, _) if sb.card.shown && !popup_open => {
            let page = sb.card.page.max(1) as isize;
            sb.card.scroll_by(-page);
            true
        }
        (KeyCode::PageDown, _) if sb.card.shown && !popup_open => {
            let page = sb.card.page.max(1) as isize;
            sb.card.scroll_by(page);
            true
        }
        // an empty composer: the arrows scroll a card longer than its box
        (KeyCode::Up, KeyModifiers::NONE)
            if empty && sb.card.shown && sb.card.max_scroll > 0 && !popup_open =>
        {
            sb.card.scroll_by(-1);
            true
        }
        (KeyCode::Down, KeyModifiers::NONE)
            if empty && sb.card.shown && sb.card.max_scroll > 0 && !popup_open =>
        {
            sb.card.scroll_by(1);
            true
        }
        (KeyCode::Char('x'), KeyModifiers::CONTROL) if !sb.cards.is_empty() => {
            if let Some(id) = sb.current_card().map(|c| c.id) {
                sb.send_input(format!("/close {}", id));
                sb.card.scroll = 0;
            }
            true
        }
        // Alt+R; '®' is Option+R on a macOS terminal that does not send
        // Option as Alt
        (KeyCode::Char('r'), KeyModifiers::ALT) | (KeyCode::Char('®'), KeyModifiers::NONE)
            if !sb.cards.is_empty() =>
        {
            answer_card(app);
            true
        }
        // no undo (book §13): say it, and how to change course
        (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
            push_event(&mut app.events, &mut app.cache, Ev::Info(NO_UNDO.into()));
            true
        }
        _ => false,
    }
}
