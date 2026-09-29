// annotations for content drafts. hover a block: ♡ ~ ✗ +. select text: note on it.
// with serve.py: notes are saved to content/notes/<page>.json (+ .md). without: localStorage + copy.
(() => {
  const PAGE = document.body.dataset.page || location.pathname.split('/').pop().replace(/\.html$/, '') || 'index';
  const API = '/__notes/' + PAGE;
  const LS = 'bise-notes:' + PAGE;
  const SEL = '[data-a], main h1, main h2, main h3, main p, main li, main tr, main blockquote';
  const MARK = { love: '♡', meh: '~', no: '✗' };
  let notes = [], server = false, showNotes = true;

  const css = `
  .an-bar{position:absolute;z-index:50;display:none;gap:2px;background:var(--raised);border:1px solid var(--faint);border-radius:8px;padding:2px;font:13px/1 "JetBrains Mono",monospace}
  .an-bar button,.an-panel button,.an-ed button,.an-sel{all:unset;cursor:pointer;padding:5px 7px;border-radius:6px;color:var(--dim)}
  .an-bar button:hover,.an-panel button:hover,.an-ed button:hover{background:var(--chip);color:var(--text)}
  .an-bar button.on{color:var(--acc)}
  .an-notes{display:block;margin:8px 0 4px;font:13px/1.6 "JetBrains Mono",monospace;letter-spacing:0;text-transform:none;font-weight:400}
  .an-hide .an-notes{display:none}
  .an-note{border-left:2px solid var(--acc);background:var(--raised);padding:6px 10px;margin:4px 0;border-radius:0 6px 6px 0;color:var(--text);white-space:pre-wrap}
  .an-note .q{color:var(--dim);font-style:italic}
  .an-note .r{color:var(--dim);margin-top:4px}
  .an-note .x{float:right;color:var(--faint);cursor:pointer;margin-left:8px}.an-note .x:hover{color:var(--acc)}
  .an-ed{margin:6px 0;display:block}
  .an-ed textarea{width:100%;box-sizing:border-box;min-height:64px;background:var(--raised);color:var(--text);border:1px solid var(--acc);border-radius:6px;padding:8px 10px;font:13px/1.6 "JetBrains Mono",monospace;resize:vertical;outline:none}
  .an-ed .row{display:flex;gap:6px;justify-content:flex-end;font-size:12px}
  .an-panel{position:fixed;right:16px;bottom:16px;z-index:60;background:var(--raised);border:1px solid var(--faint);border-radius:10px;padding:8px 10px;font:12px/1.5 "JetBrains Mono",monospace;color:var(--dim);display:flex;gap:4px;align-items:center;max-width:calc(100vw - 32px);flex-wrap:wrap}
  .an-panel b{color:var(--text);font-weight:600}.an-panel .ok{color:var(--acc)}
  .an-sel{position:absolute;z-index:55;display:none;background:var(--acc);color:var(--bg);font:12px/1 "JetBrains Mono",monospace;padding:6px 9px}
  .an-flag{outline:1px dashed var(--faint);outline-offset:4px;border-radius:2px}
  tr .an-notes{margin-top:6px}`;
  document.head.appendChild(Object.assign(document.createElement('style'), { textContent: css }));

  const blocks = () => [...document.querySelectorAll(SEL)].filter(el => !el.closest('.an-notes,.an-ed,.an-panel') && !(el.parentElement && el.parentElement.closest(SEL) && el.matches('main p, main li') && el.parentElement.closest('[data-a]') && !el.closest('[data-a]').isSameNode(el) && false));
  function keyOf(el) {
    if (el.dataset.a) return el.dataset.a;
    const sec = el.closest('[data-sec]');
    const scope = sec || document.body;
    const same = [...scope.querySelectorAll(el.tagName)].filter(e => !e.closest('.an-notes'));
    return (sec ? sec.dataset.sec : 'page') + '/' + el.tagName.toLowerCase() + same.indexOf(el);
  }
  const textOf = el => { const c = el.cloneNode(true); c.querySelectorAll('.an-notes,.an-ed').forEach(n => n.remove()); return c.innerText.trim().replace(/\s+/g, ' ').slice(0, 400); };
  const byKey = k => blocks().find(el => keyOf(el) === k);
  const host = el => (el.tagName === 'TR' ? el.lastElementChild : el);
  const esc = s => (s || '').replace(/[&<>]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' })[c]);
  const uid = () => Math.random().toString(36).slice(2, 9);

  async function load() {
    try { const r = await fetch(API, { cache: 'no-store' }); if (r.ok) { notes = await r.json(); server = true; } } catch (e) {}
    if (!server) notes = JSON.parse(localStorage.getItem(LS) || '[]');
    render();
  }
  async function save() {
    localStorage.setItem(LS, JSON.stringify(notes));
    if (server) { try { await fetch(API, { method: 'POST', body: JSON.stringify(notes) }); } catch (e) { server = false; } }
    panel();
  }

  function render() {
    document.querySelectorAll('.an-notes').forEach(n => n.remove());
    document.querySelectorAll('.an-flag').forEach(n => n.classList.remove('an-flag'));
    const general = [];
    for (const n of notes) {
      const el = n.key === 'general' ? null : byKey(n.key);
      if (!el) { general.push(n); continue; }
      if (n.verdict && !n.text) el.classList.add('an-flag');
      let box = host(el).querySelector(':scope > .an-notes');
      if (!box) { box = document.createElement('span'); box.className = 'an-notes'; host(el).appendChild(box); }
      box.appendChild(noteEl(n));
    }
    let g = document.getElementById('an-general');
    if (g) g.remove();
    if (general.length) {
      g = document.createElement('div'); g.id = 'an-general'; g.className = 'an-notes';
      g.style.cssText = 'max-width:760px;margin:40px auto';
      g.innerHTML = '<div style="color:var(--dim);margin-bottom:6px">general notes</div>';
      general.forEach(n => g.appendChild(noteEl(n)));
      (document.querySelector('main') || document.body).appendChild(g);
    }
    panel();
  }
  function noteEl(n) {
    const d = document.createElement('div'); d.className = 'an-note';
    d.innerHTML = `<span class="x" title="delete">×</span>${n.verdict ? `<span style="color:var(--acc)">${MARK[n.verdict]}</span> ` : ''}${n.sel ? `<span class="q">“${esc(n.quote)}”</span>${n.text ? '\n' : ''}` : ''}${esc(n.text)}${n.reply ? `<div class="r">↳ marketing: ${esc(n.reply)}</div>` : ''}`;
    d.querySelector('.x').onclick = () => { notes = notes.filter(m => m.id !== n.id); save(); render(); };
    return d;
  }

  function editor(el, opts = {}) {
    document.querySelectorAll('.an-ed').forEach(n => n.remove());
    const ed = document.createElement('span'); ed.className = 'an-ed';
    ed.innerHTML = `${opts.sel ? `<div style="color:var(--dim);font:italic 12px 'JetBrains Mono',monospace;margin-bottom:4px">“${esc(opts.sel)}”</div>` : ''}<textarea placeholder="ta note… (⌘↵ save · esc cancel)"></textarea><span class="row"><button data-k="cancel">cancel</button><button data-k="save" style="color:var(--acc)">save ⌘↵</button></span>`;
    const target = el ? host(el) : (document.querySelector('main') || document.body);
    target.appendChild(ed);
    const ta = ed.querySelector('textarea'); ta.focus();
    const done = ok => {
      const t = ta.value.trim();
      if (ok && t) {
        notes.push({ id: uid(), key: el ? keyOf(el) : 'general', quote: opts.sel || (el ? textOf(el) : ''), sel: !!opts.sel, text: t, verdict: '', at: new Date().toISOString() });
        save();
      }
      ed.remove(); render();
    };
    ed.querySelector('[data-k=save]').onclick = () => done(true);
    ed.querySelector('[data-k=cancel]').onclick = () => done(false);
    ta.addEventListener('keydown', e => { if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); done(true); } if (e.key === 'Escape') done(false); });
    ta.addEventListener('mousedown', e => e.stopPropagation());
  }

  // hover bar
  const bar = document.createElement('div'); bar.className = 'an-bar';
  bar.innerHTML = '<button data-v="love" title="j\'aime">♡</button><button data-v="meh" title="bof">~</button><button data-v="no" title="non">✗</button><button data-v="note" title="note">+</button>';
  document.body.appendChild(bar);
  let cur = null, hideT = 0;
  document.addEventListener('mouseover', e => {
    if (e.target.closest('.an-bar')) { clearTimeout(hideT); return; }
    const el = e.target.closest && e.target.closest(SEL);
    if (!el || el.closest('.an-notes,.an-ed,.an-panel')) { hideT = setTimeout(() => (bar.style.display = 'none'), 400); return; }
    clearTimeout(hideT); cur = el;
    const r = el.getBoundingClientRect();
    bar.style.display = 'flex';
    const left = Math.max(4, r.left + scrollX - bar.offsetWidth - 10);
    bar.style.left = (left + bar.offsetWidth + 10 > r.left + scrollX ? r.left + scrollX : left) + 'px';
    bar.style.top = (left + bar.offsetWidth + 10 > r.left + scrollX ? r.top + scrollY - bar.offsetHeight - 4 : r.top + scrollY) + 'px';
    const k = keyOf(el);
    bar.querySelectorAll('[data-v]').forEach(b => b.classList.toggle('on', notes.some(n => n.key === k && n.verdict === b.dataset.v)));
  });
  bar.addEventListener('click', e => {
    const b = e.target.closest('button'); if (!b || !cur) return;
    const v = b.dataset.v;
    if (v === 'note') return editor(cur);
    const k = keyOf(cur);
    const had = notes.find(n => n.key === k && n.verdict && !n.text);
    notes = notes.filter(n => !(n.key === k && n.verdict && !n.text));
    if (!had || had.verdict !== v) notes.push({ id: uid(), key: k, quote: textOf(cur), sel: false, text: '', verdict: v, at: new Date().toISOString() });
    save(); render();
    bar.querySelectorAll('[data-v]').forEach(x => x.classList.toggle('on', notes.some(n => n.key === k && n.verdict === x.dataset.v)));
  });

  // selection note
  const selBtn = document.createElement('button'); selBtn.className = 'an-sel'; selBtn.textContent = '✎ note';
  document.body.appendChild(selBtn);
  let selInfo = null;
  document.addEventListener('mouseup', e => {
    if (e.target.closest('.an-sel,.an-ed,.an-panel,.an-bar')) return;
    setTimeout(() => {
      const s = getSelection(); const t = s && s.toString().trim();
      if (!t || t.length < 2) { selBtn.style.display = 'none'; return; }
      const node = s.anchorNode && (s.anchorNode.nodeType === 1 ? s.anchorNode : s.anchorNode.parentElement);
      const el = node && node.closest(SEL);
      if (!el) return;
      selInfo = { el, t: t.slice(0, 300) };
      const r = s.getRangeAt(0).getBoundingClientRect();
      selBtn.style.display = 'block';
      selBtn.style.left = r.right + scrollX + 6 + 'px'; selBtn.style.top = r.top + scrollY - 30 + 'px';
    }, 0);
  });
  selBtn.addEventListener('click', () => { selBtn.style.display = 'none'; if (selInfo) editor(selInfo.el, { sel: selInfo.t }); getSelection().removeAllRanges(); });

  // panel
  const pn = document.createElement('div'); pn.className = 'an-panel'; document.body.appendChild(pn);
  function panel() {
    pn.innerHTML = `<span><b>${notes.length}</b> note${notes.length === 1 ? '' : 's'} · ${server ? '<span class="ok">saved for marketing ✓</span>' : 'local only · copy &amp; paste'}</span><button data-k="g">+ general</button><button data-k="c">copy all</button><button data-k="h">${showNotes ? 'hide' : 'show'}</button>`;
    pn.querySelector('[data-k=g]').onclick = () => editor(null);
    pn.querySelector('[data-k=h]').onclick = () => { showNotes = !showNotes; document.body.classList.toggle('an-hide', !showNotes); panel(); };
    pn.querySelector('[data-k=c]').onclick = async e => {
      const md = notes.map(n => `- ${n.verdict ? MARK[n.verdict] : '·'} [${n.key}] "${n.quote.slice(0, 140)}"${n.text ? `\n  → ${n.text}` : ''}`).join('\n');
      await navigator.clipboard.writeText(`notes · ${PAGE}\n${md}`); e.target.textContent = 'copied ✓';
    };
  }

  load();
  setInterval(async () => { // pick up marketing's replies
    if (!server || document.querySelector('.an-ed')) return;
    try { const r = await fetch(API, { cache: 'no-store' }); if (r.ok) { const fresh = await r.json(); if (JSON.stringify(fresh) !== JSON.stringify(notes)) { notes = fresh; render(); } } } catch (e) {}
  }, 5000);
})();
