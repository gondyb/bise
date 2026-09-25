#!/usr/bin/env node
// repl-ui/repl.ts — a terminal UI on top of the Bend harness REPL server.
//
// The Bend harness (runtime/repl.bend) is a TCP line server that owns the
// session; this UI is just a client: it sends command lines and renders
// the harness observations with colors.
//
//   1. start the harness:   bend runtime/repl.bend     (or ./repl)
//   2. start the UI:        node --experimental-strip-types repl-ui/repl.ts
//
// Commands are the harness ones: say, steer, notify, compact, interrupt,
// quit. Ctrl-C exits. The session lives in the harness, not here.
import net from "node:net";
import readline from "node:readline/promises";
import { stdin, stdout } from "node:process";

const args = new Map<string, string>();
for (let i = 2; i < process.argv.length; i++) {
  const a = process.argv[i]!;
  if (a === "--host") args.set("host", process.argv[++i]!);
  else if (a === "--port") args.set("port", process.argv[++i]!);
}
const HOST = args.get("host") ?? "127.0.0.1";
const PORT = Number(args.get("port") ?? 7700);

// ANSI
const dim = (s: string) => `\x1b[2m${s}\x1b[0m`;
const bold = (s: string) => `\x1b[1m${s}\x1b[0m`;
const blue = (s: string) => `\x1b[34m${s}\x1b[0m`;
const green = (s: string) => `\x1b[32m${s}\x1b[0m`;
const red = (s: string) => `\x1b[31m${s}\x1b[0m`;
const yellow = (s: string) => `\x1b[33m${s}\x1b[0m`;
const cyan = (s: string) => `\x1b[36m${s}\x1b[0m`;
const magenta = (s: string) => `\x1b[35m${s}\x1b[0m`;

// render one observation line from the harness
function renderObs(o: string): string | null {
  if (o === "turn_started") return dim("-- tour demarre --");
  if (o.startsWith("assistant: ")) return blue(bold("assistant: " + o.slice(11)));
  if (o.startsWith("tool_started #")) return `  ${cyan("outil #" + o.slice(14) + " demarre")}`;
  const m = o.match(/^tool_finished #(\d+) (ok|failed)$/);
  if (m) return m[2] === "ok"
    ? `  ${green("outil #" + m[1] + " termine")}`
    : `  ${red("outil #" + m[1] + " echoue")}`;
  if (o.startsWith("tool_result_committed #")) return dim(`  resultat #${o.slice(23)} commit`);
  if (o.startsWith("steering_received: ")) return yellow("  steering recu: " + o.slice(19));
  if (o.startsWith("steered: ")) return yellow("  steering transmis au modele: " + o.slice(8));
  if (o.startsWith("notification_received: ")) return cyan("  notification: " + o.slice(23));
  if (o.startsWith("notification_delivered: ")) return cyan("  notification livree au modele: " + o.slice(23));
  if (o.startsWith("candidate_discarded: ")) return magenta("  candidat ecarte: " + o.slice(21));
  if (o.startsWith("compaction_started #")) return yellow(bold("  compaction #" + o.slice(20)));
  if (o.startsWith("context_compaction_failed: ")) return red("  compaction echouee: " + o.slice(28));
  if (o.startsWith("compaction_done: ")) return green("  compaction faite: " + o.slice(17));
  if (o === "null_iteration") return dim("  iteration nulle comptee");
  if (o.startsWith("turn_done: ")) {
    const t = o.slice(11);
    const label = t === "completed" ? green("tour termine")
      : t.startsWith("failed") ? red("tour echoue: " + t.slice(7))
      : red("tour interrompu");
    return bold("  " + label);
  }
  return null;
}

function renderLine(line: string): void {
  if (line === "") return;
  if (line === "--- idle") {
    console.log(dim("--- inactif"));
    return;
  }
  const obs = line.startsWith("  obs: ") ? line.slice(7) : null;
  if (obs !== null) {
    const r = renderObs(obs);
    if (r !== null) {
      console.log(r);
      return;
    }
  }
  console.log(line);
}

async function main(): Promise<void> {
  console.log(bold("bend-harness UI") + dim(`  (${HOST}:${PORT})`));
  const sock = net.connect({ host: HOST, port: PORT });
  sock.setEncoding("utf8");

  await new Promise<void>((resolve, reject) => {
    sock.once("connect", () => resolve());
    sock.once("error", (err: Error) => reject(err));
  }).catch((err: Error) => {
    console.error(red("connexion impossible: " + err.message));
    console.error(dim("lance d'abord le harness :  bend runtime/repl.bend"));
    process.exit(1);
  });
  console.log(dim("connecte. session cote harness. commandes : say / steer / notify / compact / interrupt / quit"));
  console.log(dim("say tools ...  |  say prog: call echo hi; ret"));

  let buf = "";
  let promptDue: NodeJS.Timeout | null = null;
  const prompt = (): void => {
    if (promptDue !== null) clearTimeout(promptDue);
    promptDue = setTimeout(() => rl.prompt(), 30);
  };

  sock.on("data", (chunk: string) => {
    buf += chunk;
    const parts = buf.split("\n");
    buf = parts.pop() ?? "";
    for (const l of parts) {
      renderLine(l);
      if (l === "--- idle") prompt();
    }
    prompt();
  });
  sock.on("close", () => {
    console.log(dim("le harness a ferme la connexion"));
    process.exit(0);
  });

  const rl = readline.createInterface({ input: stdin, output: stdout, prompt: bold("bend> ") });
  rl.prompt();

  rl.on("line", (line: string) => {
    const t = line.trim();
    if (t === "") {
      rl.prompt();
      return;
    }
    if (t === "exit" || t === "quit") {
      sock.write("quit\n");
      return;
    }
    sock.write(t + "\n");
  });
  rl.on("close", () => {
    sock.destroy();
    process.exit(0);
  });
  rl.on("SIGINT", () => {
    sock.destroy();
    process.exit(0);
  });
}

main();
