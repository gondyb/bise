//! The line reader (§6.2): every line of every segment, typed when it
//! can be, never stopping at a bad line.
use crate::types::{must_of, Payload, ENUM_FIELDS};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// The format major this reader knows (§6.1).
pub const FORMAT: u64 = 1;

/// A place in the log: the segment file name and its 1-based line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loc {
    pub file: String,
    pub line: usize,
}

/// One accepted event. `raw` is the line as written (without the
/// newline): a writer never rewrites it, so unknown fields survive.
#[derive(Debug, Clone)]
pub struct Event {
    pub loc: Loc,
    pub seq: u64,
    pub at: String,
    pub turn: Option<u64>,
    pub must: bool,
    pub typ: String,
    pub v: u64,
    pub data: Value,
    /// None: an unknown (type, v), or a known one whose data does not fit.
    pub payload: Option<Payload>,
    pub raw: String,
}

/// How a session may be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Open {
    Ok,
    /// an unknown `must` event: display only, no resume, no append
    ReadOnly,
    /// an unknown format major, or no session_start / segment_start
    Refused(String),
}

#[derive(Debug, Clone)]
pub struct Log {
    pub open: Open,
    pub events: Vec<Event>,
    /// not JSON, not an object, or no seq/type/v
    pub bad_lines: Vec<Loc>,
    /// the bytes after the last newline of the last segment
    pub torn: Option<Vec<u8>>,
    /// events of an unknown (type, v), skipped (kept in `events` with no payload)
    pub unknown: Vec<Loc>,
    /// known events whose data does not fit the type (skipped like unknown ones)
    pub malformed: Vec<(Loc, String)>,
    /// an unknown enum value, read as "other": (where, field)
    pub others: Vec<(Loc, String)>,
    /// seq that does not go up: the first one is kept, these are dropped
    pub seq_errors: Vec<Loc>,
    /// the segment files, oldest first
    pub files: Vec<PathBuf>,
}

impl Log {
    pub fn last_seq(&self) -> u64 {
        self.events.last().map(|e| e.seq).unwrap_or(0)
    }
    pub fn by_seq(&self, seq: u64) -> Option<&Event> {
        self.events
            .binary_search_by_key(&seq, |e| e.seq)
            .ok()
            .map(|i| &self.events[i])
    }
}

/// The segment files of a session folder, oldest first: events.NNNNNN.jsonl
/// by number, then events.jsonl.
pub fn segments(dir: &Path) -> Vec<PathBuf> {
    let mut old: Vec<(u64, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let n = name.strip_prefix("events.")?.strip_suffix(".jsonl")?.parse::<u64>().ok()?;
            Some((n, e.path()))
        })
        .collect();
    old.sort();
    let mut out: Vec<PathBuf> = old.into_iter().map(|(_, p)| p).collect();
    let cur = dir.join("events.jsonl");
    if cur.exists() {
        out.push(cur);
    }
    out
}

/// Read a session folder.
pub fn read_dir(dir: &Path) -> std::io::Result<Log> {
    let files = segments(dir);
    let mut parts = Vec::new();
    for f in &files {
        let name = f.file_name().unwrap_or_default().to_string_lossy().to_string();
        parts.push((name, std::fs::read(f)?));
    }
    let mut log = read_bytes(&parts);
    log.files = files;
    Ok(log)
}

/// Read segments given as (file name, bytes), oldest first.
pub fn read_bytes(parts: &[(String, Vec<u8>)]) -> Log {
    let mut log = Log {
        open: Open::Ok,
        events: Vec::new(),
        bad_lines: Vec::new(),
        torn: None,
        unknown: Vec::new(),
        malformed: Vec::new(),
        others: Vec::new(),
        seq_errors: Vec::new(),
        files: Vec::new(),
    };
    let n = parts.len();
    for (i, (name, bytes)) in parts.iter().enumerate() {
        let (body, torn) = match bytes.iter().rposition(|&b| b == b'\n') {
            Some(p) => (&bytes[..=p], &bytes[p + 1..]),
            None => (&bytes[..0], &bytes[..]),
        };
        if !torn.is_empty() {
            if i + 1 == n {
                log.torn = Some(torn.to_vec());
            } else {
                // an older segment is closed by a rotation: a torn end there
                // is a bad line, not a repair
                log.bad_lines.push(Loc { file: name.clone(), line: body.split(|&b| b == b'\n').count() });
            }
        }
        let mut first = true;
        // counted once: counting it per line made a read quadratic (a 2 MB
        // log took 21 s in a debug build)
        let lines = body.split(|&b| b == b'\n').count();
        for (k, line) in body.split(|&b| b == b'\n').enumerate() {
            if k + 1 == lines && line.is_empty() {
                break; // after the last newline
            }
            let loc = Loc { file: name.clone(), line: k + 1 };
            let head = first;
            first = false;
            let Some(ev) = parse_line(line, loc.clone()) else {
                log.bad_lines.push(loc);
                continue;
            };
            if head {
                if let Err(why) = check_head(&ev, i == 0) {
                    log.open = Open::Refused(why);
                    return log;
                }
            }
            if ev.seq <= log.last_seq() && !log.events.is_empty() {
                log.seq_errors.push(loc);
                continue;
            }
            if ev.payload.is_none() {
                match Payload::parse(&ev.typ, ev.v, &ev.data) {
                    None => {
                        log.unknown.push(loc.clone());
                        if ev.must {
                            log.open = Open::ReadOnly;
                        }
                    }
                    Some(Err(e)) => {
                        log.malformed.push((loc.clone(), e));
                        if ev.must || must_of(&ev.typ) {
                            log.open = Open::ReadOnly;
                        }
                    }
                    Some(Ok(_)) => unreachable!(),
                }
            } else {
                for (t, field, known) in ENUM_FIELDS {
                    if *t == ev.typ {
                        if let Some(Value::String(s)) = ev.data.get(*field) {
                            if !known.contains(&s.as_str()) {
                                log.others.push((loc.clone(), field.to_string()));
                            }
                        }
                    }
                }
            }
            log.events.push(ev);
        }
    }
    if matches!(log.open, Open::Ok) && log.events.is_empty() {
        log.open = Open::Refused("empty session log".into());
    }
    log
}

fn check_head(ev: &Event, first_segment: bool) -> Result<(), String> {
    let want = if first_segment { "session_start" } else { "segment_start" };
    if ev.typ != want {
        return Err(format!("{}: the first line is {}, not {want}", ev.loc.file, ev.typ));
    }
    match ev.data.get("format").and_then(Value::as_u64) {
        Some(FORMAT) => Ok(()),
        Some(f) => Err(format!("session written by a newer bise (format {f}): update to resume it")),
        None => Err(format!("{}: no format", ev.loc.file)),
    }
}

/// One line: the envelope and, for a known (type, v), its payload.
/// None when it is not an event at all (a bad line).
pub fn parse_line(line: &[u8], loc: Loc) -> Option<Event> {
    let raw = std::str::from_utf8(line).ok()?;
    let Value::Object(mut o) = serde_json::from_str::<Value>(raw).ok()? else {
        return None;
    };
    let seq = o.get("seq")?.as_u64()?;
    let typ = o.get("type")?.as_str()?.to_string();
    let v = o.get("v")?.as_u64()?;
    let at = o.get("at").and_then(Value::as_str).unwrap_or("").to_string();
    let turn = o.get("turn").and_then(Value::as_u64);
    let must = o.get("must").and_then(Value::as_bool).unwrap_or(false);
    let data = o.remove("data").unwrap_or(Value::Object(Map::new()));
    let payload = Payload::parse(&typ, v, &data).and_then(Result::ok);
    Some(Event { loc, seq, at, turn, must, typ, v, data, payload, raw: raw.to_string() })
}
