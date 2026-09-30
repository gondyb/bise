//! bend-plugins: Agent Plugins 1.0 (the portable base) for the Bend
//! harness. Design: projects/switchboard/docs/plugins.md.
//!
//! - `resolve`: discovery, manifest validation, precedence, components,
//!   diagnostics (pure over the file system, no process started);
//! - `state`: the enable/disable file;
//! - `report`: the human and JSON listings;
//! - `stdio`: a stdio MCP client (one child process, JSON-RPC lines);
//! - `bridge`: the per-session loopback HTTP bridge the Bend REPL calls;
//! - `cli`: the `bise plugins ...` subcommand;
//! - `import`: `bise plugins import-mcp`, another agent's MCP servers as
//!   one plugin (BISE-273).

pub mod bridge;
pub mod cli;
pub mod import;
pub mod report;
pub mod resolve;
pub mod state;
pub mod stdio;
