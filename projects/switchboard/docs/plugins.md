# Agent plugins in the harness and Switchboard

Status: implemented, 2026-09-28 (task `plugins`). Code: `rust/plugins`
(`bend-plugins`), `runtime/plugins.bend`, `runtime/skills.bend`,
`runtime/mcp.bend`, `rust/tui/src/plugins.rs`.

## Goal

Load [Agent Plugins 1.0](https://agent-plugins.org/specification)
packages: a folder with a `plugin.json`, optional `skills/` and an
optional `mcp.json`. The same plugin works in a single-agent session
(`bend-harness`) and in every Switchboard task, because both run the same
Bend REPL (`repl-live`).

Supported now (the portable base of the spec):

- discovery in a user root and a workspace root;
- `plugin.json` validation against the closed 1.0.0 schema, with stable
  diagnostic codes;
- `skills/*/SKILL.md` in the skills catalog and the `skill` tool;
- local `stdio` MCP servers from `mcp.json`: started with the session,
  their tools callable as `tools.<namespace>.<tool>`;
- a list with diagnostics: `bend-harness plugins` (CLI) and `/plugins`
  (TUI);
- enable and disable, per plugin name.

Not supported yet, reported as `plugin.component.unsupported` in the
diagnostics (the rest of the plugin still loads):

- `streamable-http` and `sse` servers in `mcp.json`;
- everything under `ai.mistral.vibe/` (hooks, agents, knowledge, views),
  `connectors.json`, `libraries.json`;
- the `ai.mistral.vibe` manifest extension (`toolNamespace`,
  `toolOverrides`, `displayName`): read by no one, reported once.

No Vibe compatibility layer: no `~/.vibe/plugins`, no Claude/Codex/Kimi
foreign formats.

## What Vibe does, and where we differ

Sources: `~/mistral/dashboard/vibe/vibe/core/plugins/` (Python resolver,
`_native.py`, `_naming.py`, `_diagnostics.py`) and the schemas and
fixtures in `vibe_sdk/harness/plugins/`.

| Topic | Vibe CLI | Bend harness |
| --- | --- | --- |
| Roots | `~/.vibe/plugins/`, `<project>/.vibe/plugins/` (trusted folders only) | `~/.agents/plugins/`, `<workspace>/.agents/plugins/` (same convention as our `.agents/skills`) |
| Precedence | project over user, same name | same |
| Same name twice in one root | both dropped (`plugin.name.collision`) | same |
| Pinning | each session copies the tree into a read-only content-addressed store | none: the plugin runs from its folder. `/reload` picks up edits |
| `PLUGIN_DATA` | per session, under the session dir | durable: `~/.bend-harness/plugin-data/<name>/` |
| Skill names | `<namespace>:<skill>` | same |
| MCP tool names | `tools.<group>.<tool>`, group = namespace, `toolOverrides` rename | `tools.<namespace>.<tool>`, no overrides; a name collision drops the later tool |
| Tool exposure | `programmatic` by default, overridable | programmatic only: `search_tool_functions`, `run_typescript`, direct call by dotted name (like our connectors) |
| stdio transport | persistent clients in the Vibe MCP registry | persistent servers behind a per-session loopback bridge (below) |
| HTTP transports | yes | not yet |
| Enable / disable | none at plugin level (mounting = enabling); `disabled` per MCP server in config | `bend-harness plugins disable <name>`, stored in `~/.bend-harness/plugins.json` |
| Hooks, knowledge, agents, views, connectors, libraries | loaded (some broken locally, see the plugin-creator skill) | listed as unsupported |
| Inspect | `/plugins`, `/reload-plugins` (behind `--experimental-harness`) | `/plugins` (TUI), `bend-harness plugins` (CLI), `/reload` re-resolves |

## Where the code goes

The Bend REPL cannot hold a child process open: Base `Process.run`
takes stdin as one string and waits for the exit. A stdio MCP server
needs a pipe that stays open. The REPL already speaks MCP Streamable
HTTP (`runtime/mcp.bend`, for the Mistral connectors). So the work splits:

- **Rust, new crate `rust/plugins` (`bend-plugins`)**: the resolver
  (discovery, validation, diagnostics), the enable state, a stdio MCP
  client, and a small loopback HTTP bridge. It is linked into
  `bend-harness` (subcommand `plugins`) and into `bend-tui` (`/plugins`).
- **Bend, `runtime/`**: start the bridge at live startup, read two more
  index files. No new protocol code: plugin tools go through the same
  `mcp_call` path as connector tools, with a loopback URL instead of the
  gateway URL.

```text
repl-live ──start──▶ bend-harness plugins serve --dir D --parent <repl pid> --workspace W
    │                   ├─ resolve plugins (user root, workspace root, enable state)
    │                   ├─ spawn each stdio server, initialize + tools/list (10 s)
    │                   ├─ write D/skills-index.txt, D/mcp-index.txt, D/report.txt
    │                   ├─ touch D/ready
    │                   └─ serve http://127.0.0.1:<p>/<token>/<plugin>/<server>
    │                        until the REPL pid is gone
    └─ mcp_call "ns.tool" ──POST (initialize, initialized, tools/call)──▶ bridge ──stdin/stdout──▶ server
```

`D` is `~/.bend-harness/run/<repl port>/plugins/`. Ports are unique among
live REPLs, so two sessions (or two Switchboard tasks) never share it.

### Resolver (`bend-plugins`)

1. **Roots.** User root: `$BEND_PLUGINS_HOME` or `~/.agents/plugins`.
   Workspace root: `<workspace>/.agents/plugins`, where the workspace is
   `$BEND_WORKDIR` (set by the Switchboard hub per task) or the REPL's
   cwd. Each direct child directory with a `plugin.json` is a candidate.
   An unreadable root gives `plugin.discovery.root_unreadable`; a missing
   one is silent.
2. **Manifest.** JSON object, closed schema: `$schema` must be the 1.0.0
   URL, `name` must match `^(?!.*(?:--|\.\.))[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$`
   (1-64 chars), optional fields typed as in the schema, `extensions` an
   object of objects, no other key. Any failure: `plugin.manifest.invalid`
   with the JSON path, and the whole plugin is dropped.
3. **Namespace.** `name` with every character outside `[A-Za-z0-9_$]`
   turned into `_`, and a leading `_` before a digit. Reserved:
   `file_system`, `process`, `self`, `skill`, `subagent`, `vibe`
   (`plugin.namespace.reserved`, fatal). Two surviving plugins with the
   same namespace: `plugin.namespace.collision`, both dropped.
4. **Precedence.** Same `name` in both roots: the workspace one wins,
   the user one is listed as `shadowed`. Same `name` twice in one root:
   both dropped (`plugin.name.collision`).
5. **Enable state.** `~/.bend-harness/plugins.json`:
   `{"disabled": ["name", ...]}`. A disabled plugin is listed, its
   components are not loaded.
6. **Skills.** Each `skills/<dir>/SKILL.md`, realpath inside the plugin
   root, with a YAML frontmatter holding a one-line `name` and
   `description`. Published as `<namespace>:<name>`. A bad one:
   `plugin.skill.invalid`, skipped.
7. **MCP.** `mcp.json` must match the 1.0.0 MCP schema (`$schema` const,
   `mcpServers` object). A bad file: `plugin.mcp.invalid`, no server
   loads. Per server: `stdio` only (`plugin.component.unsupported` for
   the HTTP types). `command` is a bare executable (looked up in `PATH`)
   or `./x` resolved inside the root. `${PLUGIN_ROOT}` and
   `${PLUGIN_DATA}` are replaced in `command`, `args`, `env` values and
   `cwd`; `cwd` must start with `./`, `${PLUGIN_ROOT}` or
   `${PLUGIN_DATA}` and stay inside it (default: the plugin root); an env
   key named `PLUGIN_ROOT`/`PLUGIN_DATA` is rejected. The server gets the
   REPL environment plus `PLUGIN_ROOT`, `PLUGIN_DATA` and its `env`.
   A bad server: `plugin.mcp.server_invalid`, skipped.
8. **Unsupported components.** Present on disk and reported, never read:
   `ai.mistral.vibe/{hooks.toml,agents,knowledge,views}`,
   `connectors.json`, `libraries.json`, and the `ai.mistral.vibe`
   manifest extension.

Every diagnostic has a code, a severity (`error` drops the plugin,
`warning` drops one component, `info`), the plugin name or root, and one
sentence. The resolver is pure over a file-system snapshot, so its unit
tests use temp dirs.

### Bridge (`bend-harness plugins serve`)

- Spawns every stdio server of every enabled plugin in parallel, sends
  `initialize` (protocol `2025-06-18`), `notifications/initialized`,
  `tools/list`. 10 s per server; a failure gives
  `plugin.mcp.connection_failed` (with the last stderr lines) and the
  server is left out. stderr goes to `D/<plugin>.<server>.log`.
- Tool names are made identifiers the same way as namespaces. Two tools
  of one plugin with the same name (from two servers):
  `plugin.tool.name_collision`, the later one is dropped.
- Writes, then touches `D/ready`:
  - `D/mcp-index.txt`: the same line format as the connector index,
    `<cid> <namespace> <tool> : #<description> | input: <schema>`, with
    `cid = http://127.0.0.1:<port>/<token>/<plugin>/<server>`;
  - `D/skills-index.txt`: `name\tdescription\tpath` lines;
  - `D/report.txt`: the human report (the same text as the CLI).
- Serves HTTP/1.1 on `127.0.0.1:0`. The path carries a random token, so
  another local process cannot call the tools without reading `D`.
  JSON-RPC over POST: `initialize` answers with the server's cached
  result, notifications answer `202`, anything else is forwarded with a
  fresh id and answered as `application/json` (60 s timeout). A server
  that died is respawned once on the next call.
- No server up (no plugin, or every server failed): it exits as soon
  as the files are written, so no process lingers.
- Polls the REPL pid every 500 ms; when it is gone, stops the servers
  (a short grace period, then a kill) and exits. `/reload` therefore restarts the
  bridge with the new REPL, and plugin edits apply on `/reload`.

### Bend runtime

- `repl-live` startup, before the skills scan and before the system
  prompt is built (it lists the skills): `Pl.start()` runs
  one `/bin/sh` script: clear `D`, start the bridge in the background
  (`$BEND_HARNESS_BIN`, else `./bend-harness`, else the dev build under
  `rust/target`), wait for `D/ready` (at most 15 s). No binary: no
  plugins, no error. Scripted runs never start it (hermetic suites).
- `BEND_HARNESS_BIN` is set on the REPL by `bend-harness` (single agent)
  and by the hub (`Opts.exe`).
- Skills: the scan keeps the user roots in the shared index (the TUI
  `$` popup reads it). The workspace root (`$BEND_WORKDIR/.agents/skills`,
  today `$PWD`, which is the app root in Switchboard, so workspace skills
  were not found) and `D/skills-index.txt` go to a per-session index
  `~/.bend-harness/run/<port>/skills-index.txt`. The catalog and the
  `skill` tool read the session index, then the shared one.
- MCP: `mcp_call` and `search_tool_functions` read `D/mcp-index.txt`,
  then the connector index. `gateway_url(cid)` keeps a cid that starts
  with `http://127.0.0.1:` as is, and the Mistral key is not sent to it.

### UI and CLI

- `bend-harness plugins [list] [--workspace W] [--json]`: each plugin
  with scope, version, state (`loaded`, `disabled`, `shadowed`,
  `invalid`), root, skills, MCP servers, unsupported components, then the
  diagnostics. Static: it does not start servers.
- `bend-harness plugins enable|disable <name>`: edits
  `~/.bend-harness/plugins.json`; applies at the next session start or
  `/reload`.
- `/plugins` in both TUIs prints the same report. When the session's
  `D/report.txt` exists (single agent: the REPL port), it is shown, so
  MCP connection failures and tool counts appear too.

## Tests

- `cargo test -p bend-plugins`: unit tests for every diagnostic code,
  precedence, containment (a symlink out of the root), placeholder
  expansion, the enable state (`src/resolve/tests.rs`, `src/state.rs`);
  `tests/bridge.rs` runs the fixture plugin
  (`rust/plugins/tests/fixtures/hello-plugin`: skill `greet`, a
  dependency-free Python stdio server with one tool `shout`) through the
  real bridge: index files, `initialize` + `initialized` + `tools/call`
  over HTTP on one connection, a wrong token is a 404, `PLUGIN_DATA`
  written, the bridge stops with its parent; a failing server is a
  `plugin.mcp.connection_failed` diagnostic and the skill still loads.
- `cargo test -p bend-tui plugins`: `/plugins` text.
- `projects/switchboard/tests/plugins_live.py` (live model): a real
  `bend-harness --headless` session on a throwaway workspace loads
  `hello_plugin:greet` and calls `tools.hello_plugin.shout` from
  `run_typescript`.
- `projects/switchboard/tests/plugins_sb_live.py` (live model): the
  same from a Switchboard task on a throwaway hub.
- `bend PROOF.bend`, the scripted e2e suite and the TUI tmux suite stay
  green (scripted runs never start the bridge).

## Known limits

- The TUI `$` skill popup reads the shared index only: workspace and
  plugin skills work for the agent but are not offered in the popup.
- Plugin edits apply at the next session start or `/reload` (no
  file watching, no `/reload-plugins`).
- The Switchboard `/plugins` shows the static listing of the hub
  workspace, not the per-task bridge report.

## Later

- HTTP MCP servers straight from the REPL (it has the client already).
- `ai.mistral.vibe` extension: `toolNamespace`, `toolOverrides`.
- Per-server enable/disable, a `/plugins` picker with toggles.
- Plugin descriptions in the system prompt (the spec's default guidance).
- Hooks, agents, knowledge, views.
