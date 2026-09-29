//! The bise session log (docs/research/session-format.md, spec in
//! projects/switchboard/spec/session-format.ts): one folder per session,
//! an append-only `events.jsonl` of typed events.
pub mod reader;
pub mod types;

pub use reader::{read_bytes, read_dir, Event, Loc, Log, Open};
pub use types::Payload;
