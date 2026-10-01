#!/usr/bin/env node
// The computer-use bench (docs/computer-use-briefs.md, cu-sdk item 6; run
// in wave 2). 10 tasks on local test pages (pages.mjs) and 3 on public
// read-only sites (tasks.mjs).
//
//   node --max-old-space-size=1024 computer-use/bench/bench.mjs [options] [task ids...]
//
// Scripted run (the default): each task's reference program runs the way
// bise runs run_typescript: bend-jsrt (with the `computer` SDK in its
// prelude) exits 42 per tool call, the call goes to the real MCP server
// (`bise computer-use mcp`, hence the real broker, extension or helper),
// its result is recorded, the program re-runs with the results so far.
// The bench measures, per task: done or not (the task's check), tool
// calls, rounds, wall time, bytes of results (what a model would read).
// It needs a set-up stack: a browser with the bise extension connected
// (`bise computer-use status`), never the user's tabs (the agent's own
// tab group; tasks close their tabs).
//
//   --local | --public   only the local tasks, only the public ones
//   --bise <path>        the bise to run (default: bise on PATH)
//   --mcp "<cmd>"        the MCP server command (default: "<bise> computer-use mcp")
//   --jsrt <path>        bend-jsrt (default: $BEND_JSRT_BIN, else the tree's debug build)
//   --out <file>         the JSON report (default: $TMPDIR/computer-use-bench.json)
//   --prompts            print the tasks' prompts as JSON lines (the
//                        model-in-the-loop run: one agent per prompt) and exit
//   --list               the tasks, one per line
//   --serve              only serve the local pages (prints the URL) until ctrl-c
//
// The agent name the broker sees: $SB_AGENT, else "cu-bench".

import { spawn, spawnSync } from "node:child_process";
import { createServer } from "node:http";
import { mkdtempSync, writeFileSync, mkdirSync, rmSync, existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { PAGES } from "./pages.mjs";
import { TASKS } from "./tasks.mjs";

const here = path.dirname(new URL(import.meta.url).pathname);
const root = path.resolve(here, "../..");

function parseArgs(argv) {
  const o = { ids: [], kind: null, bise: "bise", mcp: null, jsrt: null, out: null, mode: "run" };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--local") o.kind = "local";
    else if (a === "--public") o.kind = "public";
    else if (a === "--bise") o.bise = argv[++i];
    else if (a === "--mcp") o.mcp = argv[++i];
    else if (a === "--jsrt") o.jsrt = argv[++i];
    else if (a === "--out") o.out = argv[++i];
    else if (a === "--prompts") o.mode = "prompts";
    else if (a === "--list") o.mode = "list";
    else if (a === "--serve") o.mode = "serve";
    else if (a.startsWith("--")) throw new Error(`unknown option ${a}`);
    else o.ids.push(a);
  }
  return o;
}

async function servePages() {
  const server = createServer((req, res) => {
    const name = path.basename(new URL(req.url, "http://x").pathname) || "search.html";
    const body = PAGES[name];
    if (!body) { res.writeHead(404); res.end("not found"); return; }
    res.writeHead(200, { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" });
    res.end(body);
  });
  await new Promise((r) => server.listen(0, "127.0.0.1", r));
  return { base: `http://127.0.0.1:${server.address().port}/`, close: () => server.close() };
}

// ---- the MCP server, over stdio (JSON-RPC lines) ----
class Mcp {
  constructor(cmd, env) {
    const [bin, ...args] = cmd;
    this.p = spawn(bin, args, { env, stdio: ["pipe", "pipe", "inherit"] });
    this.buf = "";
    this.id = 0;
    this.wait = new Map();
    this.p.stdout.on("data", (d) => {
      this.buf += d.toString();
      let i;
      while ((i = this.buf.indexOf("\n")) >= 0) {
        const line = this.buf.slice(0, i).trim();
        this.buf = this.buf.slice(i + 1);
        if (!line) continue;
        let m;
        try { m = JSON.parse(line); } catch { continue; }
        const w = this.wait.get(m.id);
        if (w) { this.wait.delete(m.id); w(m); }
      }
    });
    this.dead = new Promise((r) => this.p.on("exit", (code) => r(code)));
  }
  request(method, params) {
    const id = ++this.id;
    const reply = new Promise((r) => this.wait.set(id, r));
    this.p.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
    return Promise.race([reply, this.dead.then((c) => ({ error: { message: `the MCP server exited (${c})` } }))]);
  }
  notify(method, params) {
    this.p.stdin.write(JSON.stringify({ jsonrpc: "2.0", method, params }) + "\n");
  }
  async init() {
    const r = await this.request("initialize", { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "cu-bench", version: "1" } });
    if (r.error) throw new Error(`MCP initialize: ${r.error.message}`);
    this.notify("notifications/initialized", {});
    const t = await this.request("tools/list", {});
    return (t.result?.tools || []).map((x) => x.name);
  }
  // one tool call -> { ok, text } (isError = a transport failure: the
  // runtime ends the program on it, so does the bench)
  async call(name, args) {
    const r = await this.request("tools/call", { name, arguments: args });
    if (r.error) return { ok: false, text: r.error.message };
    const text = (r.result?.content || []).map((c) => c.text || "").join("\n");
    return { ok: !r.result?.isError, text };
  }
  close() { try { this.p.stdin.end(); this.p.kill(); } catch {} }
}

// ---- one program, the replay loop of bend/runtime/main.bend ----
function jsrtRound(jsrt, dir, program, results, env) {
  const prog = path.join(dir, "prog.ts");
  const res = path.join(dir, "res.json");
  writeFileSync(prog, program);
  writeFileSync(res, JSON.stringify(results));
  const r = spawnSync(jsrt, [prog, res], { env, encoding: "utf8", maxBuffer: 64 << 20 });
  return { code: r.status, out: (r.stdout || "").trim(), err: (r.stderr || "").trim() };
}

async function runProgram({ jsrt, dir, env, mcp }, program) {
  const results = [];
  const calls = [];
  let bytes = 0;
  const t0 = Date.now();
  for (let round = 1; round <= 200; round++) {
    const r = jsrtRound(jsrt, dir, program, results, env);
    if (r.code === 0) return { done: true, out: r.out, calls, rounds: round, ms: Date.now() - t0, bytes };
    if (r.code === 43) {
      let msg = r.out;
      try { msg = JSON.parse(r.out).error; } catch {}
      return { done: false, out: msg, calls, rounds: round, ms: Date.now() - t0, bytes };
    }
    if (r.code !== 42) return { done: false, out: `bend-jsrt exit ${r.code}: ${r.err || r.out}`, calls, rounds: round, ms: Date.now() - t0, bytes };
    const req = JSON.parse(r.out);
    const tool = String(req.tool).replace(/^tools\./, "");
    if (!tool.startsWith("computer.")) return { done: false, out: `the program called ${tool}`, calls, rounds: round, ms: Date.now() - t0, bytes };
    const c0 = Date.now();
    const res = await mcp.call(tool.slice("computer.".length), req.args || {});
    calls.push({ tool, args: req.args, ms: Date.now() - c0, bytes: res.text.length, error: /^\{"error"/.test(res.text) ? JSON.parse(res.text).error.code : undefined });
    bytes += res.text.length;
    if (!res.ok) return { done: false, out: `program call failed: ${res.text}`, calls, rounds: round, ms: Date.now() - t0, bytes };
    results.push(res.text);
  }
  return { done: false, out: "more than 200 rounds", calls, rounds: 200, ms: Date.now() - t0, bytes };
}

function findJsrt(o) {
  const c = [o.jsrt, process.env.BEND_JSRT_BIN, path.join(root, "rust/jsrt/target/debug/bend-jsrt"), path.join(root, "rust/jsrt/target/release/bend-jsrt")];
  const f = c.find((p) => p && existsSync(p));
  if (!f) throw new Error("no bend-jsrt: pass --jsrt, set BEND_JSRT_BIN, or build rust/jsrt");
  return f;
}

async function main() {
  const o = parseArgs(process.argv.slice(2));
  let tasks = TASKS.filter((t) => (!o.kind || t.kind === o.kind) && (o.ids.length === 0 || o.ids.includes(t.id)));
  if (o.mode === "list") {
    for (const t of tasks) console.log(`${t.id.padEnd(14)} ${t.kind.padEnd(7)} ${t.prompt}`);
    return 0;
  }
  const pages = await servePages();
  const fill = (s) => s.replaceAll("__BASE__", pages.base);
  if (o.mode === "serve") {
    console.log(`serving the bench pages on ${pages.base} (ctrl-c to stop)`);
    await new Promise(() => {});
  }
  if (o.mode === "prompts") {
    for (const t of tasks) console.log(JSON.stringify({ id: t.id, kind: t.kind, prompt: fill(t.prompt) }));
    console.error(`(the local pages are served on ${pages.base} while this runs: ctrl-c when done)`);
    await new Promise(() => {});
  }

  const jsrt = findJsrt(o);
  const work = mkdtempSync(path.join(process.env.TMPDIR || os.tmpdir(), "cu-bench-"));
  // the SDK is in jsrt's prelude when the session's plugin index lists
  // the computer tools (rust/jsrt/src/main.rs computer_loaded)
  const runDir = path.join(work, "run");
  mkdirSync(path.join(runDir, "0/plugins"), { recursive: true });
  writeFileSync(path.join(runDir, "0/plugins/mcp-index.txt"),
    ["status", "open", "tabs", "apps", "snapshot", "screenshot", "act"].map((t) => `bench computer ${t} : #`).join("\n") + "\n");
  const env = { ...process.env, BEND_RUN_DIR: runDir, BEND_REPL_PORT: "0", BEND_IMAGE_DIR: path.join(work, "images"),
    SB_AGENT: process.env.SB_AGENT || "cu-bench", TMPDIR: work };
  const cmd = o.mcp ? o.mcp.split(/\s+/).filter(Boolean) : [o.bise, "computer-use", "mcp"];
  const mcp = new Mcp(cmd, env);
  const report = { started: new Date().toISOString(), base: pages.base, jsrt, mcp: cmd.join(" "), tasks: [] };
  let rc = 0;
  try {
    const tools = await mcp.init();
    for (const need of ["status", "open", "act", "snapshot"]) {
      if (!tools.includes(need)) throw new Error(`the MCP server has no ${need} tool (tools: ${tools.join(", ")})`);
    }
    const st = await mcp.call("status", {});
    report.status = st.text;
    console.log(`status: ${st.text.slice(0, 300)}`);
    for (const t of tasks) {
      const r = await runProgram({ jsrt, dir: work, env, mcp }, fill(t.program));
      const pass = r.done && t.check(r.out);
      if (!pass) rc = 1;
      report.tasks.push({ id: t.id, kind: t.kind, pass, calls: r.calls.length, rounds: r.rounds, ms: r.ms, bytes: r.bytes, out: r.out.slice(0, 2000), trace: r.calls });
      console.log(`${pass ? "ok  " : "FAIL"} ${t.id.padEnd(14)} ${String(r.calls.length).padStart(3)} calls ${String(r.ms).padStart(6)} ms ${String(r.bytes).padStart(7)} B  ${pass ? "" : r.out.split("\n")[0].slice(0, 160)}`);
    }
  } catch (e) {
    console.error(`bench: ${e.message}`);
    rc = 2;
  } finally {
    mcp.close();
    pages.close();
  }
  const passed = report.tasks.filter((t) => t.pass).length;
  report.summary = { passed, total: report.tasks.length,
    calls: report.tasks.reduce((a, t) => a + t.calls, 0), ms: report.tasks.reduce((a, t) => a + t.ms, 0),
    bytes: report.tasks.reduce((a, t) => a + t.bytes, 0) };
  const out = o.out || path.join(process.env.TMPDIR || os.tmpdir(), "computer-use-bench.json");
  writeFileSync(out, JSON.stringify(report, null, 2));
  console.log(`${passed}/${report.tasks.length} done, ${report.summary.calls} calls, ${report.summary.ms} ms, ${report.summary.bytes} B read · report: ${out}`);
  rmSync(work, { recursive: true, force: true });
  return rc;
}

main().then((rc) => process.exit(rc), (e) => { console.error(e); process.exit(2); });
