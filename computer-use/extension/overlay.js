// Injected (chrome.scripting) into an agent's tabs only. Two jobs:
// 1. Draw the agent's cursor (designer, design §5.1): a dark arrow with a
//    light edge and a pink glow, the agent's name in a pill; it glides to
//    the target (~150 ms), then a pink ring grows and fades (300 ms).
//    Purely visual, in a closed shadow root, pointer-events: none.
// 2. Tell the service worker when someone clicks or types in this tab; the
//    worker keeps the user's input only (takeover → the agent pauses).
(() => {
  if (window.__biseOverlay) return;
  window.__biseOverlay = true;

  let host = null, cursorEl = null, nameEl = null, shown = false;

  function build() {
    host = document.createElement("bise-cursor");
    host.setAttribute("aria-hidden", "true");
    host.style.cssText = "all: initial; position: fixed; inset: 0; pointer-events: none; z-index: 2147483647;";
    const root = host.attachShadow({ mode: "closed" });
    root.innerHTML = `
      <style>
        .c { position: fixed; left: 0; top: 0; transform: translate(-100px, -100px); transition: transform 150ms ease-out; will-change: transform; }
        .arrow { display: block; filter: drop-shadow(0 0 4px rgba(244, 166, 176, 0.9)) drop-shadow(0 0 10px rgba(244, 166, 176, 0.5)); }
        .pill { position: absolute; left: 16px; top: 18px; padding: 2px 7px 2px 6px; border-left: 2px solid #f4a6b0; border-radius: 3px;
                background: #141211; color: #ece6da; font: 500 11px/16px ui-monospace, "SF Mono", Menlo, monospace; white-space: nowrap; }
        .ring { position: fixed; width: 28px; height: 28px; margin: -14px 0 0 -14px; border: 2px solid #f4a6b0; border-radius: 50%;
                animation: ring 300ms ease-out forwards; }
        @keyframes ring { from { transform: scale(0.2); opacity: 1; } to { transform: scale(1.4); opacity: 0; } }
      </style>
      <div class="c">
        <svg class="arrow" width="18" height="22" viewBox="0 0 18 22" aria-hidden="true">
          <path d="M1.5 1.5 L1.5 17.5 L5.8 13.6 L8.6 20 L11.4 18.8 L8.7 12.5 L14.5 12.5 Z" fill="#141211" stroke="#ece6da" stroke-width="1.5" stroke-linejoin="round"/>
        </svg>
        <div class="pill"></div>
      </div>`;
    cursorEl = root.querySelector(".c");
    nameEl = root.querySelector(".pill");
    host.__root = root;
    (document.body || document.documentElement).appendChild(host);
  }

  function ring(x, y) {
    const r = document.createElement("div");
    r.className = "ring";
    r.style.left = x + "px";
    r.style.top = y + "px";
    host.__root.appendChild(r);
    setTimeout(() => r.remove(), 350);
  }

  chrome.runtime.onMessage.addListener((msg, _sender, reply) => {
    if (msg?.bise !== "cursor") return;
    if (msg.hide) {
      if (host) host.style.display = "none";
      shown = false;
      reply(true);
      return;
    }
    if (!host || !host.isConnected) build();
    host.style.display = "";
    nameEl.textContent = msg.agent;
    if (!shown) {
      // First appearance: start a little up-left of the target, then glide.
      cursorEl.style.transition = "none";
      cursorEl.style.transform = `translate(${msg.x - 40}px, ${msg.y - 30}px)`;
      cursorEl.getBoundingClientRect();
      cursorEl.style.transition = "";
      shown = true;
    }
    cursorEl.style.transform = `translate(${msg.x}px, ${msg.y}px)`;
    setTimeout(() => {
      if (msg.ring) ring(msg.x, msg.y);
      reply(true);
    }, 150);
    return true;
  });

  // Takeover: a real click or key from the user, in a tab they can see.
  const user = (e) => {
    // The worker keeps only what comes from the tab in view, outside its own actions.
    if (!e.isTrusted) return;
    try {
      chrome.runtime.sendMessage({ bise: "input", kind: e.type });
    } catch {
      // the extension was reloaded
    }
  };
  addEventListener("pointerdown", user, true);
  addEventListener("keydown", user, true);
})();
