// repl-tui/app.tsx — the Bend Harness TUI.
//
// A Vibe-CLI / OpenCode-style terminal interface over the harness REPL
// server (same wire protocol as repl-ui, richer presentation):
//   header, per-turn event feed, animated thinking state, input box with
//   history, and a status bar. The session lives in the harness; this is
//   presentation only.
//
//   npm start                    # live harness (127.0.0.1:7702)
//   npm start -- --port 7700     # scripted harness
import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { render, Box, Text, useApp, useInput } from "ink";
import net from "node:net";

// ---- cli args ----
const args = new Map<string, string>();
for (let i = 2; i < process.argv.length; i++) {
  const a = process.argv[i]!;
  if (a === "--host") args.set("host", process.argv[++i]!);
  else if (a === "--port") args.set("port", process.argv[++i]!);
}
const HOST = args.get("host") ?? "127.0.0.1";
const PORT = Number(args.get("port") ?? 7702);

const C = {
  brand: "cyan",
  dim: "gray",
  you: "magenta",
  ok: "green",
  warn: "yellow",
  err: "red",
};

// ---- one normalized event in the feed ----
type ToolState = "run" | "ok" | "fail";
type Ev =
  | { role: "you"; text: string }
  | { role: "assistant"; text: string }
  | { role: "tool"; n: string; state: ToolState }
  | { role: "turn" }
  | { role: "turndone"; text: string }
  | { role: "compact"; text: string }
  | { role: "compacted"; text: string }
  | { role: "warn"; text: string }
  | { role: "err"; text: string }
  | { role: "info"; text: string }
  | { role: "raw"; text: string };

// parse one protocol line from the harness
function parseLine(line: string): Ev | null {
  if (line === "" ) return null;
  if (line === "--- idle") return { role: "info", text: "inactif — session préservée" };
  if (line.startsWith("core rejected: ")) return { role: "err", text: line.slice(15) };
  if (!line.startsWith("  obs: ")) return { role: "raw", text: line };
  const o = line.slice(7);
  if (o === "turn_started") return { role: "turn" };
  if (o.startsWith("assistant: ")) return { role: "assistant", text: o.slice(11) };
  const ts = o.match(/^tool_started #(\d+)$/);
  if (ts !== null) return { role: "tool", n: ts[1]!, state: "run" };
  const tf = o.match(/^tool_finished #(\d+) (ok|failed)$/);
  if (tf !== null) return { role: "tool", n: tf[1]!, state: tf[2] === "ok" ? "ok" : "fail" };
  if (o.startsWith("tool_result_committed")) return null;
  if (o.startsWith("steering_received: ")) return { role: "info", text: `steering reçu : ${o.slice(19)}` };
  if (o.startsWith("steered: ")) return { role: "info", text: `steering transmis au modèle : ${o.slice(8)}` };
  if (o.startsWith("notification_received: ")) return { role: "info", text: `notification : ${o.slice(23)}` };
  if (o.startsWith("notification_delivered: ")) return { role: "info", text: `notification livrée au modèle : ${o.slice(23)}` };
  if (o.startsWith("candidate_discarded: ")) return { role: "warn", text: `candidat écarté : ${o.slice(21)}` };
  if (o.startsWith("compaction_started #")) return { role: "compact", text: `compaction ${o.slice(20)}` };
  if (o.startsWith("context_compaction_failed: ")) return { role: "err", text: `compaction échouée : ${o.slice(28)}` };
  if (o.startsWith("compaction_done: ")) return { role: "compacted", text: o.slice(17) };
  if (o === "null_iteration") return { role: "warn", text: "itération nulle comptée" };
  if (o.startsWith("turn_done: ")) {
    const t = o.slice(11);
    const label = t === "completed" ? "tour terminé"
      : t.startsWith("failed") ? `tour échoué : ${t.slice(7)}`
      : "tour interrompu";
    return { role: "turndone", text: label };
  }
  return { role: "raw", text: o };
}

// ---- one feed row ----
function EventRow({ ev }: { ev: Ev }): React.ReactElement {
  switch (ev.role) {
    case "you":
      return (
        <Box marginTop={1} paddingLeft={1}>
          <Text color={C.you} bold>{"> "}</Text>
          <Text color="white" bold>{ev.text}</Text>
        </Box>
      );
    case "assistant":
      return (
        <Box paddingLeft={1}>
          <Text color={C.brand} bold>{"  assistant "}</Text>
          <Text color="white">{ev.text}</Text>
        </Box>
      );
    case "tool": {
      const label = ev.state === "run" ? "…" : ev.state === "ok" ? "ok" : "ECHEC";
      const color = ev.state === "run" ? C.warn : ev.state === "ok" ? C.ok : C.err;
      return (
        <Box paddingLeft={4}>
          <Text dimColor>{"  outil #" + ev.n + " "}</Text>
          <Text color={color}>{label}</Text>
        </Box>
      );
    }
    case "turn":
      return (
        <Box marginTop={1}>
          <Text dimColor>{"── tour "}</Text>
          <Text dimColor>{"─".repeat(30)}</Text>
        </Box>
      );
    case "turndone":
      return (
        <Box paddingLeft={1} marginTop={1}>
          <Text dimColor>{"└ "}</Text>
          <Text color={C.dim} italic>{ev.text}</Text>
        </Box>
      );
    case "compact":
      return (
        <Box paddingLeft={1}>
          <Text color={C.warn} bold>{"  compaction "}</Text>
          <Text color={C.warn}>{ev.text}</Text>
        </Box>
      );
    case "warn":
      return (
        <Box paddingLeft={1}>
          <Text color={C.warn}>{"  ! "}</Text>
          <Text color={C.warn}>{ev.text}</Text>
        </Box>
      );
    case "err":
      return (
        <Box paddingLeft={1}>
          <Text color={C.err}>{"  x "}</Text>
          <Text color={C.err}>{ev.text}</Text>
        </Box>
      );
    case "info":
      return (
        <Box paddingLeft={1}>
          <Text color={C.dim} italic>{ev.text}</Text>
        </Box>
      );
    default:
      return (
        <Box paddingLeft={1}>
          <Text color={C.dim}>{ev.text}</Text>
        </Box>
      );
  }
}

// compaction_done needs its own case (the switch above collapsed it)
function FeedRow({ ev }: { ev: Ev }): React.ReactElement {
  if (ev.role === "compacted") {
    return (
      <Box paddingLeft={1}>
        <Text color={C.ok}>{"  résumé accepté : "}</Text>
        <Text color={C.dim} italic>{ev.text}</Text>
      </Box>
    );
  }
  return <EventRow ev={ev} />;
}

// ---- animated thinking indicator ----
function useDots(active: boolean): string {
  const [n, setN] = useState(0);
  useEffect(() => {
    if (!active) return;
    const t = setInterval(() => { setN((x) => (x + 1) % 4); }, 260);
    return () => { clearInterval(t); };
  }, [active]);
  return active ? ".".repeat(n === 0 ? 1 : n) + " ".repeat(3 - (n === 0 ? 1 : n)) : "";
}

function Thinking({ active, live }: { active: boolean; live: boolean }): React.ReactElement | null {
  const dots = useDots(active);
  if (!active) return null;
  return (
    <Box paddingLeft={1}>
      <Text color={C.brand}>{live ? "zai-glm-5-3 réfléchit" : "réponse en cours"}</Text>
      <Text color={C.brand}>{dots}</Text>
    </Box>
  );
}

// ---- the input box with history ----
function InputBox({
  onSubmit,
  disabled,
}: {
  onSubmit: (v: string) => void;
  disabled: boolean;
}): React.ReactElement {
  const [value, setValue] = useState("");
  const [history, setHistory] = useState<string[]>([]);
  const [hIdx, setHIdx] = useState(-1);
  const { exit } = useApp();

  useInput(
    (input, key) => {
    if (disabled) return;
    if (key.return) {
      const v = value.trim();
      if (v === "") return;
      setHistory((h) => [v, ...h]);
      setHIdx(-1);
      setValue("");
      if (v === "/quit" || v === "quit") {
        onSubmit("quit");
        exit();
        return;
      }
      onSubmit(v.startsWith("/") ? v.slice(1) : v);
      return;
    }
    if (key.ctrl && input === "c") {
      if (value === "") exit();
      else { setValue(""); setHIdx(-1); }
      return;
    }
    if (key.upArrow) {
      const next = Math.min(hIdx + 1, history.length - 1);
      if (next >= 0) { setHIdx(next); setValue(history[next]!); }
      return;
    }
    if (key.downArrow) {
      const next = hIdx - 1;
      if (next < 0) { setHIdx(-1); setValue(""); } else { setHIdx(next); setValue(history[next]!); }
      return;
    }
    if (key.backspace || key.delete) {
      setValue((v) => v.slice(0, -1));
      return;
    }
    if (!key.ctrl && !key.meta && input !== "") {
      setValue((v) => v + input);
    }
  });

  return (
    <Box borderStyle="round" borderColor={disabled ? "gray" : "cyan"} paddingX={1} marginTop={1}>
      <Text color={C.brand} bold>{"> "}</Text>
      <Text color="white">{value}</Text>
      <Text inverse color={C.brand}>{" "}</Text>
    </Box>
  );
}

// ---- the app ----
type Conn = "connecting" | "up" | "down";

function App(): React.ReactElement {
  const [conn, setConn] = useState<Conn>("connecting");
  const [events, setEvents] = useState<Ev[]>([]);
  const [pending, setPending] = useState(false);
  const sockRef = useRef<net.Socket | null>(null);
  const bufRef = useRef("");

  const push = useCallback((ev: Ev) => {
    setEvents((prev) => {
      // a tool finishing rewrites its running line
      if (ev.role === "tool" && ev.state !== "run") {
        const next = [...prev];
        for (let i = next.length - 1; i >= 0; i--) {
          const e = next[i]!;
          if (e.role === "tool" && e.n === ev.n) { next[i] = ev; return next; }
        }
      }
      return [...prev, ev];
    });
  }, []);

  const submit = useCallback((line: string) => {
    push({ role: "you", text: line });
    setPending(true);
    sockRef.current?.write(line + "\n");
  }, [push]);

  useEffect(() => {
    const sock = net.connect({ host: HOST, port: PORT });
    sockRef.current = sock;
    sock.setEncoding("utf8");
    sock.on("connect", () => setConn("up"));
    sock.on("error", () => setConn("down"));
    sock.on("close", () => {
      setConn("down");
      setPending(false);
    });
    sock.on("data", (chunk: string) => {
      bufRef.current += chunk;
      const parts = bufRef.current.split("\n");
      bufRef.current = parts.pop() ?? "";
      for (const l of parts) {
        if (l === "--- idle") { setPending(false); }
        const ev = parseLine(l);
        if (ev !== null) push(ev);
      }
    });
    // non-interactive stdin (pipe): fall back to line mode so the TUI
    // stays scriptable; a real terminal uses the raw-mode input box
    if (process.stdin.isTTY !== true) {
      process.stdin.setEncoding("utf8");
      const sendLine = (line: string): void => {
        const v = line.trim();
        if (v === "") return;
        push({ role: "you", text: v });
        setPending(true);
        sock.write(v + "\n");
      };
      process.stdin.on("data", (chunk: string) => {
        for (const l of chunk.split("\n")) sendLine(l);
      });
      process.stdin.on("end", () => {
        sock.write("quit\n");
        // the harness closes the connection after quit; leave then
        setTimeout(() => process.exit(0), 2500);
      });
    }

    return () => { sock.destroy(); };

  }, [push]);

  const isLive = PORT === 7702;
  const feed = useMemo(() => events.slice(-200), [events]);

  return (
    <Box flexDirection="column">
      {/* header */}
      <Box borderStyle="round" borderColor={conn === "up" ? "cyan" : "gray"} paddingX={1}>
        <Text color={C.brand} bold>{"BEND HARNESS"}</Text>
        <Text color={C.dim}>{`  ${isLive ? "zai-glm-5-3 · live" : "scripté"} · seuil compaction 800k · ${HOST}:${PORT}`}</Text>
        <Text color={conn === "up" ? C.ok : C.err}>{`  ${conn === "up" ? "connecté" : conn === "connecting" ? "connexion…" : "déconnecté"}`}</Text>
      </Box>

      {/* feed */}
      <Box flexDirection="column">
        {feed.map((ev, i) => <FeedRow key={i} ev={ev} />)}
      </Box>

      <Thinking active={pending && conn === "up"} live={isLive} />

      {/* input or notice */}
      {conn === "up" && process.stdin.isTTY === true ? (
        <InputBox onSubmit={submit} disabled={false} />
      ) : conn === "up" ? (
        <Box marginTop={1}>
          <Text dimColor>{"  (mode ligne — stdin non interactif)"}</Text>
        </Box>
      ) : (
        <Box marginTop={1}>
          <Text color={C.err}>
            {conn === "down" ? `harness injoignable sur ${HOST}:${PORT} — lance ` : "connexion…"}
            {conn === "down" ? (isLive ? "bend runtime/repl-live.bend (et live/bridge.ts)" : "bend runtime/repl.bend") : ""}
          </Text>
        </Box>
      )}

      {/* status bar */}
      <Box marginTop={1}>
        <Text dimColor>{"say <texte> · steer · notify · compact · interrupt · quit "}</Text>
        <Text dimColor>{"| ↑↓ historique | ctrl+c quitter"}</Text>
      </Box>
    </Box>
  );
}

render(<App />);
