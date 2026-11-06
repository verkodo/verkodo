// SPDX-License-Identifier: AGPL-3.0-or-later
//! Style and script of the generated pages, inlined – nothing is loaded from elsewhere.

pub const CSS: &str = r#"
:root { --bg: #fbfbfa; --fg: #1d1d1f; --muted: #6b6b70; --line: #dcdcd8; --card: #ffffff;
  --accent: #2f5fb3; --code: #f1f1ee; --open: #b3402f; --ok: #2f7d4a; }
@media (prefers-color-scheme: dark) {
  :root { --bg: #151517; --fg: #e8e8ea; --muted: #9a9aa2; --line: #333338; --card: #1f1f23;
    --accent: #8fb0ef; --code: #26262b; --open: #ef9a8f; --ok: #8fd4a6; }
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--fg);
  font: 16px/1.55 system-ui, -apple-system, "Segoe UI", sans-serif; }
main { max-width: 52rem; margin: 0 auto; padding: 2rem 1rem 6rem; }
h1 { font-size: 1.8rem; margin: 0 0 .25rem; }
h2 { font-size: 1.2rem; margin: 2rem 0 .5rem; }
h3 { font-size: 1rem; margin: 2rem 0 .5rem; color: var(--muted); }
nav a, .hint { color: var(--muted); font-size: .9rem; }
code, .lean { font: .92em/1.5 ui-monospace, "SF Mono", Menlo, monospace; }
.lean { background: var(--code); padding: .1em .35em; border-radius: 4px; white-space: pre-wrap; }
.step, .decl { border-top: 1px solid var(--line); padding: .25rem 0 1rem; }
.num { display: inline-block; min-width: 1.6rem; color: var(--muted); }
.kind { font-size: .75rem; text-transform: uppercase; letter-spacing: .06em; color: var(--muted); }
.title { font-weight: 600; }
.id { font: .75rem ui-monospace, monospace; color: var(--muted); margin-bottom: .4rem; }
.row { margin: .3rem 0; }
.key { color: var(--muted); font-size: .85rem; margin-right: .35rem; }
.src, .status { color: var(--muted); font-size: .85rem; }
.open { color: var(--open); font-weight: 600; }
.ok { color: var(--ok); font-weight: 600; }
.lean-error { font: .8rem/1.4 ui-monospace, monospace; white-space: pre-wrap; color: var(--muted); margin: .3rem 0; }
.text { margin: .4rem 0; }
.ref { color: var(--accent); text-decoration: underline dotted; text-underline-offset: 3px; cursor: help; }
.ref.local { color: inherit; }
.ref.external { color: inherit; text-decoration-style: dashed; }
details summary { cursor: pointer; color: var(--muted); font-size: .9rem; }
.pop { position: absolute; z-index: 10; max-width: min(34rem, calc(100vw - 16px)); background: var(--card);
  border: 1px solid var(--line); border-radius: 8px; padding: .6rem .8rem; font-size: .92rem;
  box-shadow: 0 8px 24px rgba(0,0,0,.18); }
.pop.tip { font-size: .85rem; white-space: pre-line; }
"#;

/// Nested hover: a card opens over a link; links inside the card open the next card; leaving closes from the
/// level the pointer is on. Escape closes all.
pub const JS: &str = r#"
(() => {
  const stack = [];
  let timer = null;
  const close = (level) => { while (stack.length > level) stack.pop().el.remove(); };
  const levelOf = (node) => { const p = node.closest && node.closest('.pop'); return p ? Number(p.dataset.level) : 0; };
  function open(anchor, level) {
    close(level);
    const el = document.createElement('div');
    el.className = 'pop';
    if (anchor.dataset.ref) {
      const t = document.getElementById('t-' + anchor.dataset.ref);
      if (!t) return;
      el.appendChild(t.content.cloneNode(true));
    } else {
      el.classList.add('tip');
      el.textContent = anchor.dataset.tip;
    }
    el.dataset.level = String(level + 1);
    document.body.appendChild(el);
    const r = anchor.getBoundingClientRect();
    const maxX = window.scrollX + document.documentElement.clientWidth - el.offsetWidth - 8;
    el.style.left = Math.max(window.scrollX + 8, Math.min(r.left + window.scrollX, maxX)) + 'px';
    el.style.top = (r.bottom + window.scrollY + 6) + 'px';
    stack.push({ el, anchor });
  }
  document.addEventListener('mouseover', (e) => {
    clearTimeout(timer);
    const a = e.target.closest && e.target.closest('.ref');
    if (a) {
      const l = levelOf(a);
      if (stack[l] && stack[l].anchor === a) { close(l + 1); return; }
      timer = setTimeout(() => open(a, l), 120);
      return;
    }
    const l = levelOf(e.target);
    timer = setTimeout(() => close(l), 450);
  });
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape') close(0); });
})();
"#;
