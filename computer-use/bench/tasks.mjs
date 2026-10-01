// The bench's tasks: 10 on the local pages (pages.mjs), 3 on public
// read-only sites. Each task has
//   prompt   what an agent gets in the model-in-the-loop run (wave 2)
//   program  a reference run_typescript program using the `computer`
//            object (__BASE__ = the local server's URL); the scripted run
//            plays it through bend-jsrt and the real MCP server
//   check    the program's output -> true when the task is done
// Programs close the tabs they open (5 tabs at most per agent).

export const TASKS = [
  {
    id: "search",
    kind: "local",
    prompt: "On __BASE__search.html, search for 'usb-c 2 m' and tell me how many results there are.",
    program: `async function main() {
  const tab = await computer.browser.open("__BASE__search.html");
  await tab.getByRole("searchbox").fill("usb-c 2 m");
  await tab.press("Enter");
  await tab.waitFor("results");
  const n = await tab.getByText(/\\d+ results/).textContent();
  await tab.close();
  return n;
}`,
    check: (out) => /^2 results$/.test(out.trim()),
  },
  {
    id: "form",
    kind: "local",
    prompt: "Subscribe Ana Lima (ana@example.com, Japan) on __BASE__form.html and tell me what the page says.",
    program: `async function main() {
  const tab = await computer.browser.open("__BASE__form.html");
  await tab.getByLabel("Name").fill("Ana Lima");
  await tab.getByLabel("Email").fill("ana@example.com");
  await tab.getByLabel("Country").select("Japan");
  await tab.getByLabel("I accept the terms").check();
  await tab.getByRole("button", { name: "Subscribe" }).click();
  const out = await tab.getByRole("status").textContent();
  await tab.close();
  return out;
}`,
    check: (out) => out.includes("Subscribed Ana Lima <ana@example.com> from Japan"),
  },
  {
    id: "auto-wait",
    kind: "local",
    prompt: "On __BASE__slow.html, load the report and tell me the revenue.",
    program: `async function main() {
  const tab = await computer.browser.open("__BASE__slow.html");
  await tab.getByRole("button", { name: "Load report" }).click();
  const r = await tab.getByText(/Revenue/).textContent();
  await tab.close();
  return r;
}`,
    check: (out) => out.includes("48 210"),
  },
  {
    id: "maybe-banner",
    kind: "local",
    prompt: "On __BASE__banner.html, show more stories (a cookie banner may be in the way) and tell me what appears.",
    program: `async function main() {
  const tab = await computer.browser.open("__BASE__banner.html");
  if (await tab.getByRole("button", { name: "Accept all" }).count()) {
    await tab.getByRole("button", { name: "Accept all" }).click();
  }
  await tab.getByRole("button", { name: "Show more" }).click();
  const out = await tab.getByText(/stories|banner/).textContent();
  await tab.close();
  return out;
}`,
    check: (out) => out.includes("3 more stories"),
  },
  {
    id: "look-alikes",
    kind: "local",
    prompt: "On __BASE__rows.html, delete the project beta, only that one, and confirm.",
    program: `async function main() {
  const tab = await computer.browser.open("__BASE__rows.html");
  await tab.getByRole("button", { name: "Delete beta" }).click();
  const log = await tab.getByRole("status").textContent();
  const left = await tab.getByRole("button", { name: /^Delete/ }).count();
  await tab.close();
  return log + " · " + left + " left";
}`,
    check: (out) => out.includes("deleted beta") && out.includes("2 left"),
  },
  {
    id: "read-table",
    kind: "local",
    prompt: "What does the 27-inch monitor cost on __BASE__table.html?",
    program: `async function main() {
  const tab = await computer.browser.open("__BASE__table.html");
  const snap = await tab.snapshot();
  await tab.close();
  const lines = snap.split("\\n");
  const i = lines.findIndex((l) => l.includes('Monitor 27'));
  return lines.slice(i, i + 3).join(" ");
}`,
    check: (out) => out.includes("329"),
  },
  {
    id: "scroll-lazy",
    kind: "local",
    prompt: "Scroll __BASE__lazy.html to the end of the feed and tell me how many posts it has.",
    program: `async function main() {
  const tab = await computer.browser.open("__BASE__lazy.html");
  for (let i = 0; i < 12; i++) {
    if (await tab.getByText(/End of feed/).count()) break;
    await tab.scroll("down", 2000);
  }
  const end = await tab.getByText(/End of feed/).textContent();
  await tab.close();
  return end;
}`,
    check: (out) => out.includes("60 posts"),
  },
  {
    id: "navigate",
    kind: "local",
    prompt: "Starting from __BASE__nav1.html, follow the docs to the install page and tell me what it says to do.",
    program: `async function main() {
  const tab = await computer.browser.open("__BASE__nav1.html");
  const r = await tab.getByRole("link", { name: /install/i }).click();
  const text = await tab.getByText(/installer/).textContent();
  await tab.close();
  return r.title + ": " + text;
}`,
    check: (out) => out.includes("install") && out.includes("restart"),
  },
  {
    id: "tabs",
    kind: "local",
    prompt: "Open __BASE__nav1.html and __BASE__nav2.html side by side, list your tabs, then close them all.",
    program: `async function main() {
  const a = await computer.browser.open("__BASE__nav1.html");
  const b = await computer.browser.open("__BASE__nav2.html");
  const mine = await computer.browser.tabs();
  const titles = mine.map((t) => t.title).sort().join(", ");
  await a.close();
  await b.close();
  const after = (await computer.browser.tabs()).length;
  return titles + " · " + after + " left";
}`,
    check: (out) => out.includes("Docs · install") && out.includes("Docs · start") && out.includes("0 left"),
  },
  {
    id: "element-shot",
    kind: "local",
    prompt: "Show me the sales chart of __BASE__chart.html and say which colours it has.",
    program: `async function main() {
  const tab = await computer.browser.open("__BASE__chart.html");
  const shot = await tab.getByRole("figure", { name: "Sales chart" }).screenshot();
  await tab.close();
  return JSON.stringify({ type: shot.type, width: shot.width, height: shot.height });
}`,
    check: (out) => {
      try {
        const s = JSON.parse(out);
        return s.type === "image" && s.width > 100 && s.height > 50;
      } catch {
        return false;
      }
    },
  },
  {
    id: "example.com",
    kind: "public",
    prompt: "What is the heading of https://example.com, and where does its link go?",
    program: `async function main() {
  const tab = await computer.browser.open("https://example.com/");
  const h = await tab.getByRole("heading").first().textContent();
  const r = await tab.getByRole("link").first().click();
  const url = tab.url;
  await tab.close();
  return h + " -> " + url;
}`,
    check: (out) => out.includes("Example Domain") && /iana\.org/.test(out),
  },
  {
    id: "wikipedia",
    kind: "public",
    prompt: "Who develops Playwright, according to its English Wikipedia page?",
    program: `async function main() {
  const tab = await computer.browser.open("https://en.wikipedia.org/wiki/Playwright_(software)");
  const text = await tab.read();
  await tab.close();
  return text.slice(0, 1500);
}`,
    check: (out) => /Microsoft/.test(out),
  },
  {
    id: "hacker-news",
    kind: "public",
    prompt: "List the titles of the top 5 stories on Hacker News right now.",
    program: `async function main() {
  const tab = await computer.browser.open("https://news.ycombinator.com/");
  const snap = await tab.snapshot({ maxNodes: 600 });
  await tab.close();
  const links = snap.split("\\n").filter((l) => l.includes('- link "')).map((l) => l.trim());
  return links.slice(0, 80).join("\\n");
}`,
    check: (out) => out.split("\n").length >= 30,
  },
];
