// The bench's local test pages (served on 127.0.0.1 by bench.mjs). Small
// on purpose: each one holds the trap a task measures (auto-wait, a
// banner that may show, two look-alike buttons, a lazy list...).

const page = (title, body, script = "") => `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>${title}</title>
<style>body{font:15px system-ui;margin:24px;max-width:720px} .row{margin:6px 0}</style></head>
<body>${body}${script ? `<script>${script}</script>` : ""}</body></html>`;

export const PAGES = {
  "search.html": page("Shop search", `
<h1>Cables</h1>
<form id="f" role="search"><input type="search" aria-label="Search products" id="q"><button>Search</button></form>
<ul id="results"></ul><p id="n"></p>`, `
const items = ["Anker USB-C cable 2 m", "Anker USB-C cable 1 m", "Belkin USB-C cable 2 m", "Generic HDMI cable", "Ugreen USB-C cable 3 m"];
document.getElementById("f").onsubmit = (e) => {
  e.preventDefault();
  const q = document.getElementById("q").value.toLowerCase().split(/\\s+/).filter(Boolean);
  const hits = items.filter((i) => q.every((w) => i.toLowerCase().includes(w)));
  document.getElementById("results").innerHTML = hits.map((h) => '<li><a href="#' + encodeURIComponent(h) + '">' + h + "</a></li>").join("");
  document.getElementById("n").textContent = hits.length + " results";
};`),

  "form.html": page("Sign up", `
<h1>Newsletter</h1>
<form id="f">
<div class="row"><label>Name <input id="name"></label></div>
<div class="row"><label>Email <input id="email" type="email"></label></div>
<div class="row"><label>Password <input id="pw" type="password" value="hunter2"></label></div>
<div class="row"><label>Country <select id="country"><option>France</option><option>Germany</option><option>Japan</option></select></label></div>
<div class="row"><label><input type="checkbox" id="terms"> I accept the terms</label></div>
<button>Subscribe</button></form><p id="out" role="status"></p>`, `
document.getElementById("f").onsubmit = (e) => {
  e.preventDefault();
  const v = (id) => document.getElementById(id).value;
  if (!document.getElementById("terms").checked) { document.getElementById("out").textContent = "Accept the terms first"; return; }
  document.getElementById("out").textContent = "Subscribed " + v("name") + " <" + v("email") + "> from " + v("country");
};`),

  "slow.html": page("Report", `
<h1>Monthly report</h1><button id="b">Load report</button><div id="r"></div>`, `
document.getElementById("b").onclick = () => setTimeout(() => {
  document.getElementById("r").innerHTML = '<p>Revenue: 48 210 €</p><button>Download PDF</button>';
}, 1500);`),

  "banner.html": page("News", `
<div id="cookies" role="dialog" aria-label="Cookies"><p>We use cookies.</p><button id="ok">Accept all</button></div>
<h1>Today</h1><button id="more">Show more</button><p id="more-out"></p>`, `
// the banner shows on even seconds only: the program must cope both ways
if (new Date().getSeconds() % 2) document.getElementById("cookies").remove();
document.getElementById("ok") && (document.getElementById("ok").onclick = () => document.getElementById("cookies").remove());
document.getElementById("more").onclick = () => {
  document.getElementById("more-out").textContent = document.getElementById("cookies") ? "blocked by the banner" : "3 more stories";
};`),

  "rows.html": page("Projects", `
<h1>Projects</h1>
<table><tbody>
<tr><td>alpha</td><td><button aria-label="Delete alpha">Delete</button></td></tr>
<tr><td>beta</td><td><button aria-label="Delete beta">Delete</button></td></tr>
<tr><td>gamma</td><td><button aria-label="Delete gamma">Delete</button></td></tr>
</tbody></table><p id="log" role="status"></p>`, `
document.querySelectorAll("button").forEach((b) => b.onclick = () => {
  const row = b.closest("tr"); const name = row.cells[0].textContent; row.remove();
  document.getElementById("log").textContent = "deleted " + name;
});`),

  "table.html": page("Prices", `
<h1>Price list</h1>
<table><thead><tr><th>Item</th><th>Price</th></tr></thead><tbody>
<tr><td>Keyboard</td><td>89 €</td></tr><tr><td>Mouse</td><td>35 €</td></tr><tr><td>Monitor 27"</td><td>329 €</td></tr><tr><td>Webcam</td><td>59 €</td></tr>
</tbody></table>`),

  // rows tall enough that the first 20 overflow any window: a feed that
  // fits on screen never scrolls, so it never loads more
  "lazy.html": page("Feed", `
<style>#feed li{height:60px}</style><h1>Feed</h1><ol id="feed"></ol><p id="end"></p>`, `
let n = 0;
const more = () => { for (let i = 0; i < 20; i++) { n++; const li = document.createElement("li"); li.textContent = "Post " + n; document.getElementById("feed").appendChild(li); }
  if (n >= 60) { document.getElementById("end").textContent = "End of feed: 60 posts"; window.onscroll = null; } };
more();
window.onscroll = () => { if (innerHeight + scrollY >= document.body.scrollHeight - 50) more(); };`),

  "nav1.html": page("Docs · start", `
<h1>Getting started</h1><p>Step one.</p><a href="nav2.html">Next: install</a>`),
  "nav2.html": page("Docs · install", `
<h1>Install</h1><p>Run the installer, then restart.</p><a href="nav1.html">Back to start</a>`),

  "chart.html": page("Dashboard", `
<h1>Dashboard</h1>
<figure aria-label="Sales chart" style="width:400px;height:200px;background:linear-gradient(90deg,#2a6 0 30%,#e93 30% 70%,#36c 70%);margin:0"></figure>
<p>Sales by region</p>`),

  "list.html": page("Tasks", `
<h1>Tasks</h1><ul>
<li><label><input type="checkbox" checked> write the brief</label></li>
<li><label><input type="checkbox"> review the PR</label></li>
<li><label><input type="checkbox"> ship it</label></li>
<li><label><input type="checkbox" checked> book the room</label></li>
</ul>`),
};
