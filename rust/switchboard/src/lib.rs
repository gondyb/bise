//! Switchboard: one main agent that routes the user's messages to task
//! sub-agents (RFC 0001), optional git worktrees per task (RFC 0002), and
//! agent-to-agent messaging (RFC 0003). The design docs live in
//! `projects/switchboard/docs/`.
//!
//! Layout (functional core, imperative shell):
//! - `model`, `router`, `wire`, `board`, `prompts`: pure data and parsing;
//! - `core`: the hub's decisions, a pure state machine
//!   (`Hub::handle(input) -> effects`), tested without processes;
//! - `worktree`: the git operations of RFC 0002;
//! - `daemon`: the imperative shell (REPL processes, sockets, journal);
//! - `transcript`: reading a thread (positions, cursors, origin);
//! - `cli`: the `sb` command the agents call through their bash tool;
//! - `client`: how a client (TUI, headless test) reaches the hub.

pub mod board;
pub mod cli;
pub mod client;
pub mod core;
pub mod daemon;
pub mod model;
pub mod paths;
pub mod prompts;
pub mod router;
pub mod transcript;
pub mod util;
pub mod wire;
pub mod worktree;
