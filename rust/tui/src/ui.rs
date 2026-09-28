//! The single-agent screen: feed slice, status row, prompt, popup and
//! hint row, drawn from `App` each frame.

use crate::*;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;
use std::time::Duration;
use unicode_width::UnicodeWidthStr;

/// The meter glyph: the level while recording, the fill spinner while
/// the last words are flushed.
pub(crate) fn voice_glyph(v: &voice::Voice) -> char {
    match v.flushing_since() {
        Some(at) if v.state() == voice::VoiceState::Flushing => {
            voice::flush_glyph(at.elapsed().as_millis())
        }
        _ => voice::peak_glyph(v.peak()),
    }
}

/// The composer's text rows while recording: the meter before the first
/// row, the rows indented after it, the text dimmed (Vibe's `recording`
/// input class).
pub(crate) fn recording_lines(lines: Vec<Line<'static>>, glyph: char) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .enumerate()
        .map(|(i, l)| {
            let lead = if i == 0 {
                Span::styled(
                    format!("{} ", glyph),
                    Style::default().fg(RECORDING).add_modifier(Modifier::BOLD),
                )
            } else {
                Span::raw("  ")
            };
            let mut spans = vec![lead];
            spans.extend(l.spans.into_iter().map(|s| {
                let st = s.style.fg(DIM);
                Span::styled(s.content, st)
            }));
            Line::from(spans)
        })
        .collect()
}

pub(crate) fn draw(app: &mut App, frame: &mut Frame) {
    let mut full = app.term.draw(frame, frame.area());
    // Switchboard: the header row on top, over the feed and the panel
    // (under 70 columns, no panel: the counts shorten)
    if let Some(h) = app.sb.as_ref().filter(|_| full.height > 1).map(|sb| sb.header(full.width, full.width < 70)) {
        frame.render_widget(Paragraph::new(h), Rect { height: 1, ..full });
        full = Rect { y: full.y + 1, height: full.height - 1, ..full };
    }
    if app.sb.is_some() {
        draw_bise(app, frame, full);
        help::draw(app, frame);
        return;
    }
    let area = full;
    // OpenCode layout: no header. Feed grows to fill, a blank row, the
    // status row, another blank row, the prompt block, a blank row,
    // then the hint row — the composer never touches the history.
    // the composer grows with its content (a pasted multi-line block),
    // capped at half the screen so the feed always survives
    // recording: the level meter takes the first 2 columns (Vibe puts
    // it in place of the prompt), the text is indented after it
    let voice_pad = if app.voice.active() { 2 } else { 0 };
    let inner_w = ((area.width as usize).saturating_sub(6 + voice_pad)).max(1);
    // rows as drawn (same width, same end-slot rule as the draw below):
    // wrapped by display width (emojis are 2 columns)
    let composer_rows = {
        let rows = editor::layout_input(&app.ed.text, inner_w);
        editor::drawn_rows(&rows, app.ed.cursor)
    };
    // the prompt block holds: 2 rows of top padding, the typed text,
    // one blank line, the meta row, 1 row of bottom padding
    let input_h = ((composer_rows + 5) as u16).min((area.height / 2).max(7));
    // the images strip (book §14) above the status row, while images are attached
    let strip_h = attach::strip_height(app).min(area.height.saturating_sub(input_h + 7));
    attach::set_model(&app.info.model);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3), // feed
            Constraint::Length(strip_h), // the images strip
            Constraint::Length(1), // respiration sous le feed
            Constraint::Length(1), // status row
            Constraint::Length(1), // respiration au-dessus du composeur
            Constraint::Length(input_h), // prompt
            Constraint::Length(1), // respiration au-dessus de l'aide
            Constraint::Length(1), // hint row
        ])
        .split(area);

    let (strip, chunks) = (chunks[1], [chunks[0], chunks[2], chunks[3], chunks[4], chunks[5], chunks[6], chunks[7]]);
    draw_feed(app, frame, chunks[0]);
    if strip.height > 0 {
        let r = Rect { x: strip.x + 4, width: strip.width.saturating_sub(6), ..strip };
        frame.render_widget(Paragraph::new(attach::strip_lines(app, r.width as usize)), r);
    }
    draw_status(app, frame, chunks[2]);
    draw_prompt(app, frame, chunks[4], voice_pad);
    draw_popup(app, frame, chunks[4]);
    let hint = if app.term.shown() { term::HINT } else { hint_text(app) };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(hint, Style::default().fg(DIM)))),
        chunks[6],
    );
    help::draw(app, frame);
}

/// The Switchboard layout (bise book §8), under the header: the feed
/// (its first row pinned to the "inside an agent" / preview line when
/// there is one) and a blank row, the agents panel on their right; then,
/// the whole width, the card box, the status row and the composer: `› `
/// and the typed text, the key hints flush right on its last row (on
/// the status row when the text leaves no room).
fn draw_bise(app: &mut App, frame: &mut Frame, area: Rect) {
    // the composer: 1 column of margin, `› ` (or the voice meter), the
    // text, 1 column of margin on the right
    let inner_w = (area.width as usize).saturating_sub(4).max(1);
    let composer_rows = {
        let rows = editor::layout_input(&app.ed.text, inner_w);
        editor::drawn_rows(&rows, app.ed.cursor)
    };
    let input_h = (composer_rows.max(1) as u16).min((area.height / 2).max(1));
    // the images strip (book §14): while images are attached, what the
    // composer and a 3-row feed leave
    let strip_h = attach::strip_height(app).min(area.height.saturating_sub(input_h + 5));
    // the card box: what the composer, the strip, a 3-row feed and the 2 fixed rows leave
    let card_h = sb::card_box_height(app, area, area.height.saturating_sub(input_h + strip_h + 5));
    attach::set_model(&app.info.model);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),          // feed | panel
            Constraint::Length(1),       // a blank row under the feed
            Constraint::Length(card_h),  // the card box (ctrl+g)
            Constraint::Length(strip_h), // the images strip, above the status row
            Constraint::Length(1),       // status row
            Constraint::Length(input_h), // composer
        ])
        .split(area);
    let (strip, chunks) = (chunks[3], [chunks[0], chunks[1], chunks[2], chunks[4], chunks[5]]);
    if strip.height > 0 {
        let r = Rect { x: strip.x + 3, width: strip.width.saturating_sub(4), ..strip };
        frame.render_widget(Paragraph::new(attach::strip_lines(app, r.width as usize)), r);
    }
    // the panel runs down to the blank row under the feed
    let top = Rect { height: chunks[0].height + chunks[1].height, ..chunks[0] };
    let (left, sb_panel) = sb::split(app, top);
    if let Some(p) = sb_panel {
        sb::draw_panel(app, frame, p);
    }
    let blank = Rect { y: chunks[1].y, height: chunks[1].height, ..left };
    let mut feed = Rect { height: chunks[0].height, ..left };
    if let Some(l) = app.sb.as_ref().and_then(|sb| sb.feed_banner()) {
        if feed.height > 3 {
            let row = Rect { x: feed.x + 1, width: feed.width.saturating_sub(2), height: 1, ..feed };
            frame.render_widget(Paragraph::new(l), row);
            feed = Rect { y: feed.y + 2, height: feed.height - 2, ..feed };
        }
    }
    draw_feed(app, frame, feed);
    // no agents yet, nothing said: the first-run text, dim, in the feed
    let first_run = app.sb.as_ref().and_then(|sb| sb.first_run());
    if let Some(text) = first_run.filter(|_| !app.events.iter().any(|e| ev_visible(e, app.debug))) {
        let w = (feed.width as usize).saturating_sub(3).clamp(1, 76);
        let mut lines: Vec<Line> = Vec::new();
        for p in text {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            lines.extend(wrap_words(p, w).into_iter().map(|l| Line::from(Span::styled(l, Style::default().fg(dim())))));
        }
        let r = Rect { x: feed.x + 1, width: feed.width.saturating_sub(2), ..feed };
        frame.render_widget(Paragraph::new(lines), r);
    }
    let status_w = draw_status(app, frame, chunks[3]);
    draw_composer(app, frame, chunks[4], inner_w);
    if card_h > 0 {
        sb::draw_card(app, frame, chunks[2]);
    } else if sb::card_full(app) {
        sb::draw_card(app, frame, chunks[0]);
    }
    draw_popup(app, frame, chunks[4]);
    // the key hints, flush right: on the composer's last row, else on
    // the status row, else not at all
    let hint = if app.term.shown() { term::HINT } else { hint_text(app) };
    let hint_w = hint.width() as u16 + 1;
    let last_w = composer_last_width(app, inner_w) as u16 + 3;
    let fits = |used: u16, row: Rect| row.height > 0 && used + hint_w + 2 <= row.width;
    let row = if fits(last_w, chunks[4]) {
        Some(Rect { y: chunks[4].y + chunks[4].height - 1, ..chunks[4] })
    } else if fits(status_w, chunks[3]) {
        Some(chunks[3])
    } else if blank.height > 0 {
        // the blank row under the feed
        Some(Rect { width: blank.width, ..blank })
    } else {
        None
    };
    if let Some(row) = row {
        let w = hint_w.min(row.width);
        let r = Rect { x: row.x + row.width - w, width: w, height: 1, ..row }.intersection(frame.area());
        let hint = if hint.width() < w as usize { hint.to_string() } else { truncate_chars(hint, w.saturating_sub(2) as usize) };
        frame.render_widget(Paragraph::new(Line::from(Span::styled(hint, Style::default().fg(dim())))), r);
    }
}

/// `s` cut at spaces into rows of at most `w` columns.
fn wrap_words(s: &str, w: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in s.split(' ') {
        if !cur.is_empty() && cur.width() + 1 + word.width() > w {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    out.push(cur);
    out
}

/// The display width of the composer's last drawn row.
fn composer_last_width(app: &App, inner_w: usize) -> usize {
    let rows = editor::layout_input(&app.ed.text, inner_w);
    let n = editor::drawn_rows(&rows, app.ed.cursor);
    let w = rows.get(n.saturating_sub(1)).map_or(0, |r| r.iter().filter(|c| !c.newline).map(|c| c.w).sum());
    // the cursor at the end of the row takes one more column
    w + 1
}

fn draw_feed(app: &mut App, frame: &mut Frame, area: Rect) {
    // ---- feed: cached wrapped rows, only the VISIBLE slice rendered ----
    // (a Paragraph over the whole history re-wraps everything each frame
    // and lags long sessions). The column keeps one column of margin on
    // each edge: the history never touches the screen border, and the
    // scrollbar gets its own gutter.
    let feed_w = (area.width as usize).saturating_sub(3).max(1);
    let area_w = feed_w;
    let area_h = area.height as usize;
    let text_area = Rect {
        x: area.x + 1,
        y: area.y,
        width: feed_w as u16,
        height: area.height,
    }
    // a 1..3-column feed: the margins leave no room (never outside it)
    .intersection(area);

    let n = app.events.len();
    if app.cache.len() < n {
        app.cache.resize_with(n, || None);
    }
    // main's replies carry `:*` in main's feed only (BISE-15)
    crate::render::set_main_feed(app.sb.as_ref().is_some_and(|sb| sb.is_main_focus()));
    let (debug, tick) = (app.debug, app.tick);
    macro_rules! rows_of {
        () => {
            &mut |i: usize| ensure_rows(&app.events, &mut app.cache, i, debug, area_w, tick)
        };
    }
    let down = app.scroll > 0;
    let mut anchor = if app.follow {
        bottom_anchor(n, area_h, rows_of!())
    } else {
        move_anchor(app.anchor, app.scroll, n, rows_of!())
    };
    app.scroll = 0;
    // the rows from the anchor down; fewer than the screen: the bottom
    let mut vis: Vec<Line> = Vec::with_capacity(area_h + 2);
    let mut vis_events: Vec<usize> = Vec::with_capacity(area_h + 2);
    let mut vis_rows: Vec<usize> = Vec::with_capacity(area_h + 2);
    let mut tail_visible = true;
    for pass in 0..2 {
        vis.clear();
        vis_events.clear();
        vis_rows.clear();
        tail_visible = true;
        let (mut i, mut skip) = anchor;
        while i < n {
            ensure_rows(&app.events, &mut app.cache, i, debug, area_w, tick);
            let rows = app.cache[i].as_ref().map(|c| &c.rows[..]).unwrap_or(&[]);
            for (ri, r) in rows.iter().enumerate().skip(skip) {
                if vis.len() >= area_h {
                    tail_visible = false;
                    break;
                }
                // the feed selection on the selection background
                match app.feed_sel.and_then(|s| s.cols(i, ri)) {
                    Some((a, b)) => vis.push(feedsel::highlight(r, a, b, SELECTION)),
                    None => vis.push(r.clone()),
                }
                vis_events.push(i);
                vis_rows.push(ri);
            }
            skip = 0;
            if !tail_visible {
                break;
            }
            i += 1;
        }
        // a full screen that shows the last row is the tail too
        if pass == 0 && vis.len() < area_h && anchor != (0, 0) {
            anchor = bottom_anchor(n, area_h, rows_of!());
            continue;
        }
        break;
    }
    if tail_visible && down && !app.follow {
        app.follow = true;
        app.unseen = 0;
    }
    // nothing above the anchor: the view shows the top of the feed
    let at_top = anchor.1 == 0
        && !app.events[..anchor.0.min(n)]
            .iter()
            .rev()
            .any(|e| ev_visible(e, app.debug));
    frame.render_widget(Paragraph::new(Text::from(vis)), text_area);

    if !(at_top && tail_visible) && n > 0 {
        // the scrollbar counts events, not rows: the rows of the whole
        // history are never summed
        let shown = vis_events.last().map_or(1, |l| l + 1 - anchor.0.min(*l));
        let pos = if tail_visible { n - 1 } else { anchor.0 };
        let mut state = ScrollbarState::new(n)
            .position(pos)
            .viewport_content_length(shown);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            area,
            &mut state,
        );
    }
    app.anchor = anchor;
    app.vis_events = vis_events;
    app.vis_rows = vis_rows;
    app.feed_x = text_area.x;
    app.feed_y = text_area.y;
    app.area_w = area_w;
    app.area_h = area_h;
    app.tail_visible = tail_visible;
}

/// A status note (flash, voice) still worth showing: younger than 2 s.
fn fresh_note(note: &Option<(String, std::time::Instant)>) -> Option<String> {
    note.as_ref()
        .filter(|(_, at)| at.elapsed() < Duration::from_secs(2))
        .map(|(t, _)| t.clone())
}

/// Draw the status row; the width of what it shows.
fn draw_status(app: &mut App, frame: &mut Frame, area: Rect) -> u16 {
    // ---- the status row (the OpenCode prompt status row): back to
    // bottom when pinned, else spinner + cwd while idle
    if !app.tail_visible {
        app.bottom_bar_rect = Some(area);
        let mut spans = vec![
            Span::styled(" ↓ back to the bottom", Style::default().fg(text())),
            Span::styled(" · end", Style::default().fg(dim())),
        ];
        if app.unseen > 0 {
            spans.push(Span::styled(
                format!(" · {} new lines", app.unseen),
                Style::default().fg(text()),
            ));
        }
        let line = Line::from(spans);
        let w = line.width() as u16;
        frame.render_widget(Paragraph::new(line), area);
        w
    } else {
        app.bottom_bar_rect = None;
        let flash = fresh_note(&app.flash);
        let voice_note = fresh_note(&app.voice_note);
        let status = if let Some(t) = voice_note {
            Line::from(vec![
                Span::styled("  ● ", Style::default().fg(RECORDING)),
                Span::styled(t, Style::default().fg(TEXT)),
            ])
        } else if let Some(t) = flash {
            Line::from(vec![
                Span::styled("  ✓ ", Style::default().fg(BRAND)),
                Span::styled(t, Style::default().fg(TEXT)),
            ])
        } else if let Some(l) = sb::status_line(app) {
            l
        } else if app.pending && app.connected {
            Line::from(vec![
                Span::styled(
                    format!("  {}", spinner_frame(app.tick / 2)),
                    Style::default().fg(BRAND),
                ),
                Span::styled(
                    format!(" {} · generating…", app.info.model),
                    Style::default().fg(DIM),
                ),
                Span::styled(" · ", Style::default().fg(DIM)),
                Span::styled("ctrl+c", Style::default().fg(TEXT)),
                Span::styled(" interrompre", Style::default().fg(DIM)),
            ])
        } else {
            // idle: a static standby dot — the spinner only moves
            // while a turn runs; between turns nothing animates
            Line::from(vec![
                Span::styled("  ● ", Style::default().fg(BRAND)),
                Span::styled(
                    format!(
                        " bend-harness · {}",
                        app.info.model
                    ),
                    Style::default().fg(DIM),
                ),
                Span::styled(" · ", Style::default().fg(DIM)),
                Span::styled("/ commandes", Style::default().fg(TEXT)),
                Span::styled(" · End: bottom · Ctrl+C: quit", Style::default().fg(DIM)),
            ])
        };
        let w = status.width() as u16;
        frame.render_widget(Paragraph::new(status), area);
        w
    }
}

fn draw_prompt(app: &mut App, frame: &mut Frame, area: Rect, voice_pad: usize) {
    // ---- the prompt: OpenCode prompt (left border ┃, element bg, meta row)
    // multi-line: newlines break rows, long rows wrap at the inner
    // width (layout_input)
    let inner = ((area.width as usize).saturating_sub(6 + voice_pad)).max(1);
    // the text area inside the block: left border + padding 3, padding
    // 2 right, 2 top; the rows above the meta row and its blank line
    let text_rows = (area.height as usize).saturating_sub(5).max(1);
    app.composer = ComposerArea {
        x: area.x + 4 + voice_pad as u16,
        y: area.y + 2,
        w: inner,
        h: text_rows,
        top: 0,
    };
    let mut input_lines: Vec<Line> = Vec::new();
    if app.ed.is_empty() && app.voice.active() {
        input_lines.push(Line::from(""));
    } else if app.ed.is_empty() {
        input_lines.push(Line::from(Span::styled(
            sb::placeholder(app).unwrap_or_else(|| "Ask anything…".to_string()),
            Style::default().fg(DIM),
        )));
    } else {
        input_lines = typed_lines(app, inner, text_rows);
    }
    if app.voice.active() {
        input_lines = recording_lines(input_lines, voice_glyph(&app.voice));
    }
    // the meta row speaks glyphs: ◆ the harness, ● connected (quiet),
    // ○ déconnecté (loud) — the normal state stays muted, only the
    // broken one raises its voice
    let meta = Line::from(vec![
        Span::styled(
            "◆ bend",
            Style::default().fg(BRAND).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", Style::default().fg(DIM)),
        Span::styled(
            app.info.model.clone(),
            Style::default().fg(TEXT),
        ),
        Span::styled(" · ", Style::default().fg(DIM)),
        Span::styled(
            if app.ed.text.contains('\n') {
                "⏎ send · ⇧⏎ new line"
            } else {
                "⇧⏎ new line"
            },
            Style::default().fg(DIM),
        ),
        Span::styled(" · ", Style::default().fg(DIM)),
        if app.connected {
            Span::styled("●", Style::default().fg(DIM))
        } else {
            Span::styled("○ disconnected", Style::default().fg(ERR))
        },
    ]);
    // one blank line between the typed text and the meta row
    input_lines.push(Line::from(""));
    input_lines.push(meta);
    let border = if app.voice.active() { RECORDING } else { BRAND };
    let prompt = Paragraph::new(input_lines).block(
        Block::default()
            .borders(Borders::LEFT)
            .border_set(SPLIT)
            .border_style(Style::default().fg(border))
            .style(Style::default().bg(ELEMENT))
            .padding(Padding::new(3, 2, 2, 1)),
    );
    frame.render_widget(prompt, area);
}

/// The composer's typed text as drawn rows, `inner` columns wide, at
/// most `text_rows` of them (scrolled to keep the cursor row in view;
/// sets `app.composer.top`).
fn typed_lines(app: &mut App, inner: usize, text_rows: usize) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    // rows by display width (emojis are 2 columns); the cursor is
    // the REVERSED grapheme, or a REVERSED space on a newline or at
    // the end of the text; the selection has the selection colors
    let rows = editor::layout_input(&app.ed.text, inner);
    let drawn = editor::drawn_rows(&rows, app.ed.cursor);
    let cursor = app.ed.cursor;
    let selection = app.ed.selection();
    let (cur_row, _) = editor::row_col(&rows, cursor);
    // taller than the box: scroll so the cursor row stays visible
    let top = (cur_row + 1).saturating_sub(text_rows);
    app.composer.top = top;
    let text_style = Style::default().fg(TEXT);
    let sel_style = Style::default().fg(TEXT).bg(SELECTION);
    for row in rows.iter().take(drawn).skip(top) {
        let mut spans: Vec<Span> = Vec::new();
        let mut buf = String::new();
        let mut buf_sel = false;
        for cell in row {
            let n = cell.text.chars().count().max(1);
            let is_cursor = if cell.newline {
                cell.ci == cursor
            } else {
                cell.ci <= cursor && cursor < cell.ci + n
            };
            let in_sel = selection.is_some_and(|(a, b)| a <= cell.ci && cell.ci < b);
            if cell.chip {
                // an image chip `▣ N` (accent, atomic): its own span
                if !buf.is_empty() {
                    let st = if buf_sel { sel_style } else { text_style };
                    spans.push(Span::styled(std::mem::take(&mut buf), st));
                }
                buf_sel = in_sel;
                let mut st = attach::chip_style();
                if in_sel {
                    st = st.bg(SELECTION);
                }
                if is_cursor {
                    st = st.add_modifier(Modifier::REVERSED);
                }
                spans.push(Span::styled(attach::chip_text(cell.text), st));
                continue;
            }
            if is_cursor || buf_sel != in_sel {
                if !buf.is_empty() {
                    let st = if buf_sel { sel_style } else { text_style };
                    spans.push(Span::styled(std::mem::take(&mut buf), st));
                }
                buf_sel = in_sel;
            }
            if is_cursor {
                // a pending dead key (Option+e…): its accent, marked,
                // before the cursor, like macOS; on a full row (no
                // column left) it takes the cursor cell instead, so
                // the row never overflows and the cursor stays seen
                let marked = Style::default().fg(BRAND).add_modifier(Modifier::UNDERLINED);
                let row_w: usize = row.iter().map(|c| c.w).sum();
                match app.ed.pending_dead() {
                    Some(acc) if row_w + 1 > inner => spans.push(Span::styled(
                        acc.to_string(),
                        marked.add_modifier(Modifier::REVERSED),
                    )),
                    acc => {
                        if let Some(acc) = acc {
                            spans.push(Span::styled(acc.to_string(), marked));
                        }
                        spans.push(Span::styled(
                            cell.text.to_string(),
                            text_style.add_modifier(Modifier::REVERSED),
                        ));
                    }
                }
            } else if !cell.newline {
                buf.push_str(cell.text);
            } else if in_sel {
                // a selected newline shows as one selected blank
                buf.push(' ');
            }
        }
        if !buf.is_empty() {
            spans.push(Span::styled(buf, if buf_sel { sel_style } else { text_style }));
        }
        out.push(Line::from(spans));
    }
    out
}

/// The Switchboard composer: `› ` (dim) then the typed text, one row
/// per wrapped row, continuation rows indented under the text; while
/// recording, the level meter takes the place of `›`. An empty
/// composer shows only the cursor (or a read-only note).
fn draw_composer(app: &mut App, frame: &mut Frame, area: Rect, inner: usize) {
    let text_rows = (area.height as usize).max(1);
    app.composer = ComposerArea {
        x: area.x + 3,
        y: area.y,
        w: inner,
        h: text_rows,
        top: 0,
    };
    let mut rows = if app.ed.is_empty() && !app.voice.active() {
        let note = sb::placeholder(app).unwrap_or_default();
        let mut spans = vec![Span::styled(" ", Style::default().fg(text()).add_modifier(Modifier::REVERSED))];
        if !note.is_empty() {
            spans.push(Span::styled(format!(" {}", note), Style::default().fg(dim())));
        }
        vec![Line::from(spans)]
    } else if app.ed.is_empty() {
        vec![Line::from("")]
    } else {
        typed_lines(app, inner, text_rows)
    };
    let lead = if app.voice.active() {
        Span::styled(
            format!("{} ", voice_glyph(&app.voice)),
            Style::default().fg(accent()).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(format!("{} ", G_YOU), Style::default().fg(dim()))
    };
    for (i, l) in rows.iter_mut().enumerate() {
        let mut spans = vec![Span::raw(" "), if i == 0 { lead.clone() } else { Span::raw("  ") }];
        spans.extend(l.spans.drain(..).map(|s| {
            if app.voice.active() {
                let st = s.style.fg(dim());
                Span::styled(s.content, st)
            } else {
                s
            }
        }));
        *l = Line::from(spans);
    }
    frame.render_widget(Paragraph::new(rows), area);
}

fn draw_popup(app: &App, frame: &mut Frame, prompt: Rect) {
    // ---- slash-command popup (OpenCode autocomplete: split border,
    // backgroundMenu, primary selection)
    let matches = popup_items(app);
    if !matches.is_empty() {
        let n = matches.len().min(8) as u16;
        let w = if matches[0].closable { 72u16 } else { 56u16 }.min(prompt.width);
        let sel_i = app.popup_sel.min(matches.len() - 1);
        let top = popup_top(sel_i, matches.len(), 8);
        let area = Rect {
            x: prompt.x,
            y: prompt.y.saturating_sub(n + 2),
            width: w,
            height: n + 2,
        }
        // a short terminal: the prompt sits near the top, the popup
        // would hang below the screen (ratatui panics outside its buffer)
        .intersection(frame.area());
        frame.render_widget(Clear, area);
        let lines: Vec<Line> = matches
            .iter()
            .enumerate()
            .skip(top)
            .take(8)
            .map(|(i, c)| {
                let sel = i == sel_i;
                let (name_style, desc_style) = if sel {
                    (
                        Style::default()
                            .bg(BRAND)
                            .fg(ON_BRAND)
                            .add_modifier(Modifier::BOLD),
                        Style::default().bg(BRAND).fg(ON_BRAND),
                    )
                } else {
                    (Style::default().fg(TEXT), Style::default().fg(DIM))
                };
                let mut spans = Vec::new();
                if let Some((g, color)) = c.mark {
                    let st = if sel {
                        Style::default().bg(BRAND).fg(ON_BRAND)
                    } else {
                        Style::default().fg(color)
                    };
                    spans.push(Span::styled(format!(" {}", g), st));
                }
                // columns, not chars: an emoji mark is 2 columns wide
                let mark_w = c.mark.map(|(g, _)| g.width() + 1).unwrap_or(0);
                // a long path keeps its end (the file name) in view
                let label = truncate_left(&c.label, (w as usize).saturating_sub(mark_w + 4));
                spans.push(Span::styled(format!(" {} ", label), name_style));
                let room = (w as usize).saturating_sub(label.width() + mark_w + 6);
                spans.push(Span::styled(truncate_chars(&c.desc, room), desc_style));
                Line::from(spans)
            })
            .collect();
        frame.render_widget(
            Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT)
                    .border_set(SPLIT)
                    .border_style(Style::default().fg(BORDER_ACTIVE))
                    .style(Style::default().bg(ELEMENT)),
            ),
            area,
        );
    }
}

/// `s` in at most `max` columns: its end, after a `…` when cut.
pub(crate) fn truncate_left(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    let mut out: Vec<char> = Vec::new();
    let mut used = 1; // the `…`
    for ch in s.chars().rev() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + cw > max {
            break;
        }
        used += cw;
        out.push(ch);
    }
    if max == 0 {
        return String::new();
    }
    std::iter::once('…').chain(out.into_iter().rev()).collect()
}

pub(crate) fn hint_text(app: &App) -> &'static str {
    // ---- hint row (the OpenCode prompt right hint row)
    if app.voice.state() == voice::VoiceState::Recording {
        "recording · any key stops · esc/ctrl+c cancel"
    } else if app.voice.state() == voice::VoiceState::Flushing {
        "transcribing the last words… · esc/ctrl+c cancel"
    } else if popup_open(app) && files::token(&app.ed.text, app.ed.cursor).is_some() {
        "⏎/tab insert · ⏎/tab/→ open a folder · ← up · ↑↓ select · esc close"
    } else if !app.pending && app.sb.is_some() && attach::strip_height(app) > 0 {
        // images attached (book §14): how to add more
        attach::STRIP_HINT
    } else if let Some(h) = sb::hint(app) {
        h
    } else if app.pending {
        "⏎ steer · Tab queue · Ctrl+C interrupt · / commands · End bottom"
    } else {
        "⏎ send · shift+⏎/ctrl+j new line · / commands · ctrl+o open/close all · ctrl+c quit"
    }
}
