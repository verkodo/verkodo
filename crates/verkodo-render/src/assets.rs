// SPDX-License-Identifier: AGPL-3.0-or-later
//! Style, script, font and mark of the generated pages – nothing is loaded from elsewhere. A site written by
//! `write_site` links them as files under `assets/`; a single page (the preview of the web editor) carries style and
//! script inline. The look is verkodo/brand's: warm paper or night, the face in ink or paper white, titanium grey as the
//! one colour beside it, straight edges, Crimson Pro (SIL OFL 1.1) for reading, the system's sans for the controls.

/// The font, as files of a site.
pub const FONTS: &[(&str, &[u8])] = &[
    ("CrimsonPro-Regular.ttf", include_bytes!("../assets/fonts/CrimsonPro-Regular.ttf")),
    ("CrimsonPro-SemiBold.ttf", include_bytes!("../assets/fonts/CrimsonPro-SemiBold.ttf")),
    ("OFL.txt", include_bytes!("../assets/fonts/OFL.txt")),
];

/// The mark of verkodo for light and dark grounds, copied unchanged from verkodo/brand (CC BY-ND 4.0).
pub const MARKS: &[(&str, &str)] = &[
    ("verkodo-mark-on-light.svg", include_str!("../assets/verkodo-mark-on-light.svg")),
    ("verkodo-mark-on-dark.svg", include_str!("../assets/verkodo-mark-on-dark.svg")),
];

/// The font faces; `prefix` is where the font files lie relative to the style sheet.
pub fn font_faces(prefix: &str) -> String {
    [("400", "Regular"), ("600", "SemiBold")]
        .iter()
        .map(|(w, f)| {
            format!(
                "@font-face {{ font-family: \"Crimson Pro\"; font-weight: {w}; font-style: normal; font-display: swap; \
                 src: url(\"{prefix}CrimsonPro-{f}.ttf\") format(\"truetype\"); }}\n"
            )
        })
        .collect()
}

pub const CSS: &str = r#"
:root {
  --ground: #F7F4EC; --panel: #FFFFFF; --sunk: #EFEBE1; --text: #14202B; --muted: #4A525C; --faint: #6E757D;
  --titan: #646A70; --line: #DCD6C8; --code: #EFEBE1; --ok: #2E6B45; --open: #9A3B2B; --mark: rgba(100,106,112,.14);
  --serif: "Crimson Pro", Georgia, "Times New Roman", serif;
  --sans: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
  --mono: ui-monospace, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace;
  --top: 56px; color-scheme: light;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --ground: #0D1117; --panel: #12171E; --sunk: #161C24; --text: #E6EDF3; --muted: #B4BAC1; --faint: #8A9199;
    --titan: #8F959B; --line: #222A33; --code: #161C24; --ok: #7FC79A; --open: #E79A8C; --mark: rgba(143,149,155,.16);
    color-scheme: dark;
  }
}
:root[data-theme="dark"] {
  --ground: #0D1117; --panel: #12171E; --sunk: #161C24; --text: #E6EDF3; --muted: #B4BAC1; --faint: #8A9199;
  --titan: #8F959B; --line: #222A33; --code: #161C24; --ok: #7FC79A; --open: #E79A8C; --mark: rgba(143,149,155,.16);
  color-scheme: dark;
}
.on-dark { display: none !important; }
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) .on-dark { display: inline-block !important; }
  :root:not([data-theme="light"]) .on-light { display: none !important; }
}
:root[data-theme="dark"] .on-dark { display: inline-block !important; }
:root[data-theme="dark"] .on-light { display: none !important; }

* { box-sizing: border-box; }
html { scroll-padding-top: calc(var(--top) + 16px); }
body { margin: 0; background: var(--ground); color: var(--text); font: 18px/1.6 var(--serif);
  -webkit-font-smoothing: antialiased; }
a { color: inherit; text-decoration: underline; text-decoration-color: var(--titan); text-decoration-thickness: 1px;
  text-underline-offset: 3px; }
a:hover { text-decoration-color: var(--text); }
::selection { background: var(--titan); color: var(--ground); }
:focus-visible { outline: 2px solid var(--titan); outline-offset: 2px; }
h1, h2, h3 { font-family: var(--serif); font-weight: 600; line-height: 1.2; }
h1 { font-size: 2.2rem; margin: .2rem 0 .4rem; }
h2 { font-size: 1.45rem; margin: 0 0 .3rem; }
h3 { font-size: 1.1rem; margin: 1.4rem 0 .4rem; }
code, .lean, .mono { font-family: var(--mono); font-size: .8em; }
.lean { background: var(--code); padding: .1em .4em; border-radius: 3px; white-space: pre-wrap; line-height: 1.6; }
.ui, .chip, .key, .crumbs, .meta, nav.side, aside.rail, .top, .btn, .results, .kind, .id, .src, .status, .hint,
  .progress, .stat, .tabs { font-family: var(--sans); }

/* the bar on top */
.top { position: sticky; top: 0; z-index: 20; height: var(--top); display: flex; align-items: center; gap: 16px;
  padding: 0 20px; background: var(--ground); border-bottom: 1px solid var(--line); }
.brand { display: flex; align-items: center; gap: 10px; text-decoration: none; font: 600 1.25rem var(--serif); }
.brand img { width: 26px; height: 26px; }
.search { position: relative; flex: 1; max-width: 560px; margin-left: auto; }
.search input { width: 100%; font: 15px var(--sans); color: var(--text); background: var(--panel);
  border: 1px solid var(--line); border-radius: 6px; padding: 8px 12px 8px 34px; }
.search::before { content: "⌕"; position: absolute; left: 11px; top: 50%; transform: translateY(-52%);
  color: var(--faint); font: 18px var(--sans); pointer-events: none; }
.search input:focus { outline: none; border-color: var(--titan); }
.results { position: absolute; top: calc(100% + 6px); left: 0; right: 0; background: var(--panel);
  border: 1px solid var(--line); border-radius: 8px; box-shadow: 0 12px 32px rgba(0,0,0,.18); padding: 6px;
  max-height: 70vh; overflow: auto; }
.results a { display: flex; align-items: baseline; gap: 10px; padding: 7px 10px; border-radius: 5px;
  text-decoration: none; font-size: 15px; }
.results a .where { margin-left: auto; color: var(--faint); font-size: 12px; white-space: nowrap; overflow: hidden;
  text-overflow: ellipsis; max-width: 45%; }
.results a.on, .results a:hover { background: var(--mark); }
.results .none { padding: 10px; color: var(--faint); font-size: 14px; }
.icon { font: 16px var(--sans); color: var(--muted); background: none; border: 1px solid var(--line);
  border-radius: 6px; width: 36px; height: 36px; cursor: pointer; }
.icon:hover { color: var(--text); border-color: var(--titan); }
#menu { display: none; }

/* three columns: the tree, the text, the rail */
.layout { display: grid; grid-template-columns: 260px minmax(0, 1fr) 240px; max-width: 1440px; margin: 0 auto; }
.layout.wide { grid-template-columns: 260px minmax(0, 1fr); }
nav.side, aside.rail { position: sticky; top: var(--top); align-self: start; max-height: calc(100vh - var(--top));
  overflow: auto; padding: 20px 16px 40px; font-size: 14px; }
nav.side { border-right: 1px solid var(--line); }
aside.rail { padding-left: 20px; }
main.content { padding: 28px clamp(16px, 4vw, 56px) 96px; min-width: 0; }
main.content > * { max-width: 760px; }
main.home > * { max-width: 980px; }

.tree, .tree ul { list-style: none; margin: 0; padding: 0; }
.tree ul { padding-left: 12px; border-left: 1px solid var(--line); margin-left: 6px; }
.tree details > summary { list-style: none; cursor: pointer; padding: 4px 6px; color: var(--muted);
  font-weight: 600; font-size: 12px; text-transform: uppercase; letter-spacing: .06em; }
.tree details > summary::-webkit-details-marker { display: none; }
.tree details > summary::before { content: "▸"; display: inline-block; width: 1em; color: var(--faint); }
.tree details[open] > summary::before { content: "▾"; }
.tree .group > details > summary { text-transform: none; letter-spacing: 0; font-weight: 500; font-size: 13px; }
.tree a { display: flex; align-items: center; gap: 6px; padding: 4px 6px; border-radius: 5px; text-decoration: none;
  color: var(--text); }
.tree a:hover { background: var(--mark); }
.tree a.here { background: var(--mark); font-weight: 600; }
.tree a .seen { margin-left: auto; color: var(--faint); font-size: 11px; visibility: hidden; }
.tree a.visited .seen { visibility: visible; }

.rail h4 { margin: 0 0 8px; font-size: 11px; text-transform: uppercase; letter-spacing: .07em; color: var(--faint); }
.rail ol, .rail ul { list-style: none; margin: 0 0 24px; padding: 0; }
.rail li a { display: block; padding: 3px 0 3px 10px; border-left: 2px solid transparent; color: var(--muted);
  text-decoration: none; line-height: 1.35; }
.rail li a:hover { color: var(--text); }
.rail li a.on { border-left-color: var(--titan); color: var(--text); }

/* pieces of an article */
.crumbs { font-size: 13px; color: var(--faint); margin-bottom: 4px; }
.crumbs a { color: var(--faint); text-decoration: none; }
.crumbs a:hover { color: var(--text); }
.meta { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; margin: 8px 0 24px; font-size: 13px;
  color: var(--muted); }
.chip { display: inline-flex; align-items: center; gap: 4px; font-size: 12px; line-height: 1; padding: 4px 8px;
  border: 1px solid var(--line); border-radius: 999px; color: var(--muted); background: var(--panel);
  white-space: nowrap; text-decoration: none; }
a.chip:hover { border-color: var(--titan); color: var(--text); }
.chip.ok { color: var(--ok); border-color: color-mix(in srgb, var(--ok) 40%, var(--line)); }
.chip.open { color: var(--open); border-color: color-mix(in srgb, var(--open) 40%, var(--line)); }
.chip.kind-tag { text-transform: uppercase; letter-spacing: .06em; font-size: 10.5px; }
.lead { font-size: 1.15rem; color: var(--muted); margin: 0 0 1rem; }
.decl { background: var(--panel); border: 1px solid var(--line); border-radius: 10px; padding: 18px 22px;
  margin: 0 0 18px; }
.decl > header { display: flex; align-items: baseline; gap: 10px; flex-wrap: wrap; margin-bottom: 6px; }
.decl > header h2 { margin: 0; }
.decl > header .id { margin-left: auto; }
.id { font-size: 12px; color: var(--faint); font-family: var(--mono); }
.row { margin: .35rem 0; }
.key { color: var(--faint); font-size: 13px; margin-right: .4rem; }
.src, .status { color: var(--faint); font-size: 13px; }
.src { margin-top: .3rem; }
.ok { color: var(--ok); font-weight: 600; }
.open { color: var(--open); font-weight: 600; }
.lean-error { font: 12px/1.4 var(--mono); white-space: pre-wrap; color: var(--muted); margin: .3rem 0; }
.text { margin: .4rem 0; }
.ref { color: inherit; text-decoration: underline dotted; text-decoration-color: var(--titan);
  text-underline-offset: 3px; cursor: pointer; }
.ref:hover { text-decoration-style: solid; }
.ref.local, .ref.external { cursor: help; }
.ref.external { text-decoration-style: dashed; }
details summary { cursor: pointer; color: var(--muted); font: 14px var(--sans); }
ol, ul { padding-left: 1.4rem; }
.links { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; margin-top: 12px; padding-top: 10px;
  border-top: 1px dashed var(--line); font-size: 13px; }
.links .key { margin-right: 2px; }
.parts { margin-top: 12px; border-top: 1px solid var(--line); }
.part { border-bottom: 1px solid var(--line); }
.part > summary { display: flex; gap: 10px; align-items: baseline; padding: 8px 2px; font: 15px var(--serif);
  color: var(--text); list-style: none; }
.part > summary::-webkit-details-marker { display: none; }
.part > summary::before { content: "▸"; color: var(--faint); font: 12px var(--sans); }
.part[open] > summary::before { content: "▾"; }
.part > summary .kind { font-size: 10.5px; text-transform: uppercase; letter-spacing: .06em; color: var(--faint);
  min-width: 4.5rem; }
.part > summary .id { margin-left: auto; }
.part > .body { padding: 0 0 12px 22px; }
.hint { color: var(--faint); font-size: 13px; }
:target { scroll-margin-top: calc(var(--top) + 16px); }
.decl:target, .part:target { box-shadow: 0 0 0 2px var(--titan); }

/* the home page */
.hero { padding: 24px 0 8px; }
.hero h1 { font-size: 2.6rem; }
.hero p { font-size: 1.2rem; color: var(--muted); max-width: 640px; }
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 16px; margin: 12px 0 32px; }
.tile { display: block; background: var(--panel); border: 1px solid var(--line); border-radius: 10px;
  padding: 16px 18px; text-decoration: none; }
a.tile:hover { border-color: var(--titan); }
.tile h3 { margin: 6px 0 4px; font-size: 1.2rem; }
.tile p { margin: 0; color: var(--muted); font-size: 15px; }
.tile .stat { font-size: 12px; color: var(--faint); margin-top: 10px; display: flex; gap: 10px; flex-wrap: wrap; }
.tile ul { margin: 8px 0 0; padding: 0; list-style: none; font-size: 15px; }
.tile li { padding: 3px 0; border-top: 1px solid var(--line); }
.tile li:first-child { border-top: 0; }
.tile li a { text-decoration: none; display: flex; gap: 8px; }
.tile li a:hover { text-decoration: underline; }
.bar { height: 6px; border-radius: 3px; background: var(--sunk); overflow: hidden; margin-top: 12px; }
.bar > span { display: block; height: 100%; width: 0; background: var(--titan); transition: width .3s; }
.section-title { font: 600 12px var(--sans); text-transform: uppercase; letter-spacing: .08em; color: var(--faint);
  margin: 28px 0 0; }
#resume:empty { display: none; }

/* a path: reading mode */
.steps-nav { counter-reset: step; }
.steps-nav li a { display: flex; gap: 8px; align-items: baseline; }
.steps-nav li a::before { counter-increment: step; content: counter(step); min-width: 1.4em; color: var(--faint);
  font-variant-numeric: tabular-nums; }
.steps-nav li.done a::before { content: "✓"; color: var(--ok); }
.progress { display: flex; align-items: center; gap: 10px; font-size: 13px; color: var(--muted); margin: 6px 0 22px; }
.progress .bar { flex: 1; max-width: 280px; margin: 0; }
.step { position: relative; }
.step > header .num { font: 600 13px var(--sans); color: var(--faint); min-width: 1.6rem; }
.step-foot { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; margin-top: 14px; padding-top: 12px;
  border-top: 1px dashed var(--line); }
.btn { font: 14px var(--sans); color: var(--text); background: var(--panel); border: 1px solid var(--line);
  border-radius: 6px; padding: 6px 12px; cursor: pointer; text-decoration: none; display: inline-flex; gap: 6px;
  align-items: center; }
.btn:hover { border-color: var(--titan); }
.btn.got[aria-pressed="true"] { color: var(--ok); border-color: color-mix(in srgb, var(--ok) 45%, var(--line)); }
.step-foot .next { margin-left: auto; }
.step.done { border-color: color-mix(in srgb, var(--ok) 35%, var(--line)); }
.finished { display: none; text-align: center; padding: 22px; color: var(--muted); }
.finished.show { display: block; }

/* versions of a provision */
.versions { margin-top: 12px; font: 13px var(--sans); }
.versions > ol { list-style: none; padding: 0; margin: 6px 0 0; display: flex; flex-wrap: wrap; gap: 0; }
.versions > ol > li { position: relative; padding: 0 22px 0 16px; display: flex; flex-direction: column; gap: 2px; }
.versions > ol > li::before { content: ""; position: absolute; left: 0; top: 7px; width: 9px; height: 9px;
  border-radius: 50%; border: 2px solid var(--titan); background: var(--panel); }
.versions > ol > li:not(:last-child)::after { content: ""; position: absolute; left: 11px; right: 4px; top: 11px;
  border-top: 2px solid var(--line); }
.versions > ol > li.here::before { background: var(--titan); }
.versions > ol > li a { position: relative; z-index: 1; background: var(--panel); padding-right: 6px; }
.versions .when { color: var(--faint); font-size: 12px; }
.versions .now-tag { display: none; color: var(--ok); font-size: 12px; font-weight: 600; }
.versions > ol > li.now .now-tag { display: inline; }
.versions .diff { margin-top: 8px; }
.versions .diff ul { list-style: none; padding: 0; margin: 6px 0 0; font: 14px var(--serif); }
.versions .diff li { padding: 3px 0; }

/* context: a module among the others */
.hood { display: flex; flex-direction: column; align-items: stretch; gap: 4px; margin-bottom: 24px; }
.hood .tier { display: flex; flex-direction: column; gap: 4px; }
.hood .wire { text-align: center; color: var(--faint); line-height: 1; }
.node { display: block; padding: 6px 10px; border: 1px solid var(--line); border-radius: 8px; background: var(--panel);
  text-decoration: none; color: var(--text); font: 13px/1.3 var(--sans); }
a.node:hover { border-color: var(--titan); }
.node.here { border-color: var(--titan); font-weight: 600; }

/* the map of the knowledge */
.map { position: relative; display: grid; grid-template-columns: repeat(var(--cols), minmax(180px, 1fr));
  gap: 18px 48px; margin: 12px 0 28px; overflow-x: auto; padding: 4px; }
.map .col { display: flex; flex-direction: column; gap: 12px; justify-content: center; position: relative; z-index: 1; }
.map .node { font-size: 14px; padding: 8px 12px; }
.map .node .area { display: block; font-size: 10.5px; text-transform: uppercase; letter-spacing: .06em;
  color: var(--faint); margin-bottom: 2px; }
.map .node.lit { border-color: var(--titan); box-shadow: 0 0 0 1px var(--titan); }
.map .wires { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; z-index: 0; overflow: visible; }
.map .wires path { fill: none; stroke: var(--line); stroke-width: 1.5; }
.map .wires path.lit { stroke: var(--titan); stroke-width: 2; }

/* the calculator */
.calc { margin-top: 14px; }
.calc .panel { margin-top: 10px; border: 1px solid var(--line); border-radius: 8px; background: var(--ground); }
.calc .form { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 10px 16px;
  padding: 14px 16px; }
.field { display: flex; flex-direction: column; gap: 4px; font: 13px var(--sans); color: var(--muted); min-width: 0; }
.field.check { flex-direction: row; align-items: center; gap: 8px; }
.field.wide, .field.rec { grid-column: 1 / -1; }
.field.rec { border: 1px dashed var(--line); border-radius: 6px; padding: 8px 12px 12px; margin: 0;
  display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 8px 14px; }
.field.rec legend { padding: 0 4px; color: var(--faint); font-size: 12px; }
.field input[type=text], .field input[type=date], .field select, .field textarea { font: 14px var(--sans);
  color: var(--text); background: var(--panel); border: 1px solid var(--line); border-radius: 5px; padding: 6px 8px;
  width: 100%; }
.field input:focus, .field select:focus, .field textarea:focus { outline: none; border-color: var(--titan); }
.field .num { display: flex; align-items: center; gap: 6px; }
.field .unit { color: var(--faint); }
.calc .out { border-top: 1px solid var(--line); padding: 12px 16px 14px; }
.calc .res { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 6px 16px; margin: 6px 0 10px;
  font-size: 17px; }
.calc .res dt { color: var(--text); }
.calc .res dd { margin: 0; font: 600 15px var(--mono); text-align: right; font-variant-numeric: tabular-nums; }
.calc .trace { list-style: none; padding: 0; margin: 8px 0 0; font-size: 15px; }
.calc .trace li { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 12px; padding: 4px 0;
  border-bottom: 1px dotted var(--line); }
.calc .trace .val { font: 13px var(--mono); text-align: right; font-variant-numeric: tabular-nums; }
.calc .trace .unknown, .calc .res .unknown { color: var(--faint); font-weight: 400; }
.calc .trace .why { grid-column: 1 / -1; font: 12px var(--sans); color: var(--faint); }
.calc .msg { padding: 12px 16px; font: 14px var(--sans); color: var(--muted); }
.calc h4 { margin: 0; font: 600 11px var(--sans); text-transform: uppercase; letter-spacing: .07em;
  color: var(--faint); }

/* hover cards */
.pop { position: absolute; z-index: 40; width: min(34rem, calc(100vw - 16px)); background: var(--panel);
  border: 1px solid var(--line); border-radius: 10px; padding: 12px 16px; font-size: 16px; line-height: 1.5;
  box-shadow: 0 12px 32px rgba(0,0,0,.22); }
.pop .kind { font-size: 10.5px; text-transform: uppercase; letter-spacing: .07em; color: var(--faint); }
.pop .title { font-weight: 600; font-size: 1.1rem; }
.pop .id { margin-bottom: .4rem; }
.pop .go { display: block; margin-top: 8px; font: 13px var(--sans); color: var(--muted); }
.pop.tip { font: 13px/1.45 var(--sans); white-space: pre-line; }

/* narrower screens: the rail goes, then the tree becomes a drawer */
@media (max-width: 1180px) {
  .layout { grid-template-columns: 240px minmax(0, 1fr); }
  aside.rail { display: none; }
}
@media (max-width: 820px) {
  .layout, .layout.wide { grid-template-columns: minmax(0, 1fr); }
  #menu { display: inline-block; }
  .brand span { display: none; }
  nav.side { position: fixed; top: var(--top); left: 0; bottom: 0; width: min(320px, 86vw); max-height: none;
    background: var(--ground); z-index: 30; transform: translateX(-100%); transition: transform .2s;
    box-shadow: 0 0 40px rgba(0,0,0,.25); }
  body.menu nav.side { transform: none; }
  main.content { padding: 20px 16px 80px; }
  h1, .hero h1 { font-size: 1.8rem; }
  .decl { padding: 14px 14px; }
  .decl > header .id, .part > summary .id, .part > summary .kind { display: none; }
}
@media print {
  .top, nav.side, aside.rail, .step-foot { display: none; }
  .layout { display: block; }
  .decl { break-inside: avoid; border: 0; padding: 0; }
}
"#;

/// The page's behaviour, all optional – without it the pages are plain linked HTML. Reads `window.VK` (a site's
/// `assets/data.js`, the cards apart in `assets/cards.js`) or the JSON in `#vk-data` (a single page): `cards` (id → card markup), `search` (entries [id,
/// label, kind, url, where]), `forms`, `calc` and `words`. A fixed text: the static web editor allows it by its hash. Progress and the theme stay in this
/// browser (localStorage); nothing leaves it.
pub const JS: &str = r#"
(() => {
  const VK = window.VK || (() => {
    const d = document.getElementById('vk-data');
    try { return d ? JSON.parse(d.textContent) : null; } catch (e) { return null; }
  })() || { search: [], words: {} };
  const W = (k) => (VK.words && VK.words[k]) || k;
  const where = new Map((VK.search || []).map((e) => [e[0], e]));
  const store = {
    get(k, d) { try { const v = localStorage.getItem('verkodo:' + k); return v === null ? d : JSON.parse(v); } catch (e) { return d; } },
    set(k, v) { try { localStorage.setItem('verkodo:' + k, JSON.stringify(v)); } catch (e) {} },
  };
  const root = document.documentElement;
  const touch = window.matchMedia('(pointer: coarse)').matches;

  // theme: light, dark or the system's
  const applyTheme = (t) => { if (t === 'light' || t === 'dark') root.dataset.theme = t; else delete root.dataset.theme; };
  applyTheme(store.get('theme', 'system'));
  const themeBtn = document.getElementById('theme');
  if (themeBtn) themeBtn.addEventListener('click', () => {
    const dark = root.dataset.theme ? root.dataset.theme === 'dark' : window.matchMedia('(prefers-color-scheme: dark)').matches;
    const t = dark ? 'light' : 'dark';
    store.set('theme', t); applyTheme(t);
  });
  const menu = document.getElementById('menu');
  if (menu) menu.addEventListener('click', () => document.body.classList.toggle('menu'));

  // pages read: a quiet mark in the tree
  const page = document.body.dataset.page;
  const seen = store.get('seen', {});
  if (page) { seen[page] = 1; store.set('seen', seen); }
  document.querySelectorAll('.tree a[data-page]').forEach((a) => { if (seen[a.dataset.page]) a.classList.add('visited'); });

  // nested hover cards: a card opens over a term; terms in the card open the next one
  const stack = [];
  let timer = null, wanted = null;
  const close = (level) => { while (stack.length > level) stack.pop().el.remove(); };
  const levelOf = (n) => { const p = n.closest && n.closest('.pop'); return p ? Number(p.dataset.level) : 0; };
  // the cards: in the data of a single page, on a site in assets/cards.js, loaded when a card is first wanted
  let cardsLoad = null;
  const cards = () => VK.cards || window.VK_CARDS;
  function loadCards() {
    if (!cardsLoad) cardsLoad = new Promise((done) => {
      const s = document.createElement('script');
      s.src = 'assets/cards.js'; s.onload = done; s.onerror = done;
      document.head.appendChild(s);
    });
    return cardsLoad;
  }
  function open(anchor, level) {
    const id = anchor.dataset.ref;
    if (id && !cards()) {
      if (!cardsLoad) loadCards().then(() => { if (cards() && wanted === anchor) open(anchor, level); });
      wanted = anchor;
      return;
    }
    close(level);
    const el = document.createElement('div');
    el.className = 'pop';
    if (id) {
      const card = cards()[id];
      if (!card) return;
      el.innerHTML = card;
      // links in a card carry their id; where they lead comes from the search
      el.querySelectorAll('a.ref[data-ref]:not([href])').forEach((a) => {
        const e = where.get(a.dataset.ref);
        if (e) a.setAttribute('href', e[3]); else { a.setAttribute('role', 'button'); a.tabIndex = 0; }
      });
      if (anchor.getAttribute('href') && touch) {
        const go = document.createElement('a');
        go.className = 'go'; go.href = anchor.getAttribute('href'); go.textContent = W('go') + ' →';
        el.appendChild(go);
      }
    } else if (anchor.dataset.tip) {
      el.classList.add('tip'); el.textContent = anchor.dataset.tip;
    } else return;
    el.dataset.level = String(level + 1);
    document.body.appendChild(el);
    const r = anchor.getBoundingClientRect();
    const maxX = window.scrollX + document.documentElement.clientWidth - el.offsetWidth - 8;
    el.style.left = Math.max(window.scrollX + 8, Math.min(r.left + window.scrollX, maxX)) + 'px';
    const below = r.bottom + 6 + el.offsetHeight < window.innerHeight || r.top < el.offsetHeight + 6;
    el.style.top = (below ? r.bottom + window.scrollY + 6 : r.top + window.scrollY - el.offsetHeight - 6) + 'px';
    stack.push({ el, anchor });
  }
  if (!touch) {
    document.addEventListener('mouseover', (e) => {
      clearTimeout(timer);
      const a = e.target.closest && e.target.closest('.ref');
      if (a) {
        const l = levelOf(a);
        if (stack[l] && stack[l].anchor === a) { close(l + 1); return; }
        timer = setTimeout(() => open(a, l), 140);
        return;
      }
      const l = levelOf(e.target);
      wanted = null;
      timer = setTimeout(() => close(l), 450);
    });
  }
  document.addEventListener('click', (e) => {
    const a = e.target.closest && e.target.closest('.ref');
    if (a && (touch || !a.getAttribute('href'))) {
      e.preventDefault();
      const l = levelOf(a);
      if (stack[l] && stack[l].anchor === a) close(l); else open(a, l);
      return;
    }
    if (!(e.target.closest && e.target.closest('.pop'))) close(0);
  });
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') { close(0); hideResults(); document.body.classList.remove('menu'); }
  });

  // search: every declaration by its name, its id and where it stands
  const q = document.getElementById('q');
  const box = document.getElementById('results');
  const norm = (s) => s.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/ß/g, 'ss');
  const entries = (VK.search || []).map((e) => ({ e, label: norm(e[1]), id: norm(e[0]), where: norm(e[4] || '') }));
  let hits = [], cur = 0;
  function score(x, words) {
    let s = 0;
    for (const w of words) {
      if (x.label.startsWith(w)) s += 6;
      else if (x.label.includes(' ' + w)) s += 4;
      else if (x.label.includes(w)) s += 3;
      else if (x.id.includes(w)) s += 2;
      else if (x.where.includes(w)) s += 1;
      else return 0;
    }
    return s + (['concept', 'norm', 'theorem', 'path'].includes(x.e[2]) ? 1 : 0);
  }
  const esc = (s) => s.replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
  function hideResults() { if (box) box.hidden = true; }
  function show() {
    const words = norm(q.value).split(/\s+/).filter(Boolean);
    if (!words.length) { hideResults(); return; }
    hits = entries.map((x) => [score(x, words), x]).filter((p) => p[0] > 0).sort((a, b) => b[0] - a[0]).slice(0, 14).map((p) => p[1].e);
    cur = 0;
    box.innerHTML = hits.length
      ? hits.map((h, i) => `<a href="${esc(h[3])}" class="${i === 0 ? 'on' : ''}"><span class="chip kind-tag">${esc(W(h[2]))}</span><span>${esc(h[1])}</span><span class="where">${esc(h[4] || '')}</span></a>`).join('')
      : `<div class="none">${esc(W('nothing'))}</div>`;
    box.hidden = false;
  }
  if (q && box) {
    q.addEventListener('input', show);
    q.addEventListener('focus', () => { if (q.value) show(); });
    q.addEventListener('keydown', (e) => {
      const links = box.querySelectorAll('a');
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        if (!links.length) return;
        links[cur].classList.remove('on');
        cur = (cur + (e.key === 'ArrowDown' ? 1 : links.length - 1)) % links.length;
        links[cur].classList.add('on'); links[cur].scrollIntoView({ block: 'nearest' });
      } else if (e.key === 'Enter' && links[cur]) { e.preventDefault(); location.href = links[cur].getAttribute('href'); }
    });
    document.addEventListener('keydown', (e) => {
      if (e.key === '/' && document.activeElement !== q && !/input|textarea|select/i.test(document.activeElement.tagName)) {
        e.preventDefault(); q.focus(); q.select();
      }
    });
    document.addEventListener('click', (e) => { if (!e.target.closest('.search')) hideResults(); });
  }

  // the rail follows the reader
  const toc = [...document.querySelectorAll('.rail .toc a')];
  if (toc.length && 'IntersectionObserver' in window) {
    const byId = new Map(toc.map((a) => [decodeURIComponent(a.hash.slice(1)), a]));
    const io = new IntersectionObserver((list) => {
      for (const en of list) if (en.isIntersecting) {
        toc.forEach((a) => a.classList.remove('on'));
        const a = byId.get(en.target.id); if (a) a.classList.add('on');
      }
    }, { rootMargin: '-20% 0px -70% 0px' });
    byId.forEach((_, id) => { const el = document.getElementById(id); if (el) io.observe(el); });
  }

  // paths: steps marked as understood, the share done, where to go on
  const pathId = document.body.dataset.path;
  const done = (p) => store.get('path:' + p, []);
  function paint(p, n) {
    const d = done(p).filter((i) => i < n);
    const pct = n ? Math.round(100 * d.length / n) : 0;
    document.querySelectorAll(`[data-progress="${CSS.escape(p)}"]`).forEach((el) => {
      const bar = el.querySelector('.bar > span'); if (bar) bar.style.width = pct + '%';
      const t = el.querySelector('.count'); if (t) t.textContent = `${d.length} / ${n} ${W('understood')}`;
    });
    return d;
  }
  if (pathId) {
    const steps = [...document.querySelectorAll('.step')];
    const n = steps.length;
    const refresh = () => {
      const d = paint(pathId, n);
      steps.forEach((s, i) => {
        const on = d.includes(i);
        s.classList.toggle('done', on);
        const b = s.querySelector('.got'); if (b) b.setAttribute('aria-pressed', String(on));
        const li = document.querySelector(`.steps-nav li[data-step="${i}"]`); if (li) li.classList.toggle('done', on);
      });
      const fin = document.querySelector('.finished'); if (fin) fin.classList.toggle('show', d.length === n && n > 0);
    };
    steps.forEach((s, i) => {
      const b = s.querySelector('.got');
      if (b) b.addEventListener('click', () => {
        let d = done(pathId);
        d = d.includes(i) ? d.filter((x) => x !== i) : [...d, i];
        store.set('path:' + pathId, d);
        const paths = store.get('paths', {}); paths[pathId] = { label: document.title, at: Date.now(), href: location.pathname.split('/').pop() }; store.set('paths', paths);
        refresh();
        if (d.includes(i) && steps[i + 1]) steps[i + 1].scrollIntoView({ behavior: 'smooth', block: 'start' });
      });
    });
    refresh();
  }
  document.querySelectorAll('[data-progress]').forEach((el) => paint(el.dataset.progress, Number(el.dataset.steps || 0)));
  // the calculator: a norm computed in this browser by verkodo as WebAssembly, every step shown
  let engine = null;
  async function start() {
    if (engine) return engine;
    if (!window.VK_SOURCES) {
      await new Promise((ok, no) => {
        const s = document.createElement('script');
        s.src = 'assets/sources.js'; s.onload = ok; s.onerror = no; document.head.appendChild(s);
      });
    }
    const { instance } = await WebAssembly.instantiate(await (await fetch('assets/verkodo.wasm')).arrayBuffer(), {});
    const x = instance.exports, enc = new TextEncoder(), dec = new TextDecoder();
    const call = (op, req) => {
      const b = enc.encode(JSON.stringify(req));
      const at = x.vk_buffer(b.length);
      new Uint8Array(x.memory.buffer, at, b.length).set(b);
      const n = x[op]();
      return JSON.parse(dec.decode(new Uint8Array(x.memory.buffer, x.vk_address(), n)));
    };
    call('vk_load', window.VK_SOURCES);
    engine = call;
    return call;
  }
  const termLink = (id, label) => {
    const e = where.get(id);
    return `<a class="ref" data-ref="${esc(id)}"${e ? ` href="${esc(e[3])}"` : ''}>${esc(label || (e && e[1]) || id.split('.').pop())}</a>`;
  };
  function field(spec, path) {
    const name = esc(spec.l || spec.n);
    const tag = spec.j ? ` <span class="chip">${esc(W('judgement'))}</span>` : '';
    const at = `data-in="${esc(path)}" data-k="${esc(spec.k)}"`;
    switch (spec.k) {
      case 'bool': return `<label class="field check"><input type="checkbox" ${at}><span>${name}${tag}</span></label>`;
      case 'enumeration':
        return `<label class="field"><span>${name}${tag}</span><select ${at}>${(spec.v || []).map((v) => `<option value="${esc(v[0])}">${esc(v[1] || v[0])}</option>`).join('')}</select></label>`;
      case 'date': return `<label class="field"><span>${name}${tag}</span><input type="date" ${at}></label>`;
      case 'record':
        return `<fieldset class="field rec"><legend>${name}</legend>${(spec.f || []).map((f) => field(f, path + '.' + f.n)).join('')}</fieldset>`;
      case 'list':
        return `<label class="field wide"><span>${name}${tag}</span><textarea rows="3" ${at} placeholder="${esc('[{' + (spec.f || []).map((f) => `"${f.n}": …`).join(', ') + '}]')}"></textarea></label>`;
      default: {
        const unit = spec.k === 'money' ? '€' : spec.u || '';
        return `<label class="field"><span>${name}${tag}</span><span class="num"><input type="text" inputmode="decimal" placeholder="0" ${at}>${unit ? `<span class="unit">${esc(unit)}</span>` : ''}</span></label>`;
      }
    }
  }
  function read(box, spec, path) {
    if (spec.k === 'record') {
      const o = {};
      for (const f of spec.f || []) {
        const v = read(box, f, path + '.' + f.n);
        o[f.n] = f.k === 'bool' ? v === 'true' : f.k === 'record' ? JSON.parse(v) : v;
      }
      return JSON.stringify(o);
    }
    const el = box.querySelector(`[data-in="${CSS.escape(path)}"]`);
    if (!el) return '';
    if (spec.k === 'bool') return String(el.checked);
    if (spec.k === 'list') return el.value.trim() || '[]';
    if (spec.k === 'date' || spec.k === 'enumeration') return el.value;
    return el.value.trim() || '0';
  }
  // a value as people write it: numbers grouped in the page's language, money with two decimals and "€", truth values
  // as words, values of enumerations by their label
  const lang = root.lang || 'de';
  function show(v, k) {
    if (v === 'true' || v === 'false') return W(v === 'true' ? 'yes' : 'no');
    const n = Number(String(v).replace(/\s*EUR$/, ''));
    if (String(v).trim() !== '' && Number.isFinite(n)) {
      if (k === 'money') {
        const whole = Number.isInteger(n);
        return n.toLocaleString(lang, { minimumFractionDigits: whole ? 0 : 2, maximumFractionDigits: 2 }) + '\u00a0€';
      }
      return n.toLocaleString(lang, { maximumFractionDigits: 6 });
    }
    const e = where.get(String(v));
    return e ? e[1] : v;
  }
  function openCalc(box) {
    const id = box.dataset.norm, form = (VK.forms || {})[id];
    if (!form) return;
    box.innerHTML = `<div class="panel"><div class="form">${form.inputs.map((i) => field(i, i.n)).join('')}</div><div class="out"><p class="msg">${esc(W('loading'))}</p></div></div>`;
    const out = box.querySelector('.out');
    let t = null;
    const run = async () => {
      let call;
      try { call = await start(); } catch (e) { out.innerHTML = `<p class="msg">${esc(W('calc_failed'))}</p>`; return; }
      const inputs = {};
      for (const i of form.inputs) inputs[i.n] = read(box, i, i.n);
      const steps = call('vk_eval', { norm: id, inputs }) || [];
      const by = new Map(steps.map((s) => [s.id, s]));
      const val = (s) => s ? `<span class="${s.known ? '' : 'unknown'}">${esc(s.known ? show(s.value, (form.types || {})[s.id]) : s.value)}</span>` : '';
      const res = form.results.map((r) => by.get(r)).filter(Boolean);
      const rest = steps.filter((s) => s.kind !== 'input' && s.kind !== 'judgement');
      out.innerHTML = `<h4>${esc(W('results'))}</h4><dl class="res">${res.map((s) => `<dt>${termLink(s.id, s.label)}</dt><dd>${val(s)}</dd>`).join('')}</dl>`
        + `<details><summary>${esc(W('trace'))} · ${rest.length}</summary><ol class="trace">${rest.map((s) => {
          const hit = (s.exceptions || []).findIndex((e) => e[1]);
          const why = hit >= 0 ? `<span class="why">${esc(W('applied'))}: ${esc(W('unless'))} ${hit + 1}</span>` : '';
          return `<li><span>${termLink(s.id, s.label)}</span><span class="val">${val(s)}</span>${why}</li>`;
        }).join('')}</ol></details>`;
    };
    box.addEventListener('input', () => { clearTimeout(t); t = setTimeout(run, 200); });
    box.addEventListener('change', () => { clearTimeout(t); t = setTimeout(run, 50); });
    run();
  }
  if (VK.calc) {
    document.querySelectorAll('.calc[data-norm]').forEach((box) => {
      if (!(VK.forms || {})[box.dataset.norm]) return;
      box.hidden = false;
      // opened as a file, the browser does not load the WebAssembly: say so instead of a button that fails
      if (location.protocol === 'file:') { box.innerHTML = `<p class="msg">${esc(W('calc_file'))}</p>`; return; }
      box.innerHTML = `<button class="btn" type="button">▶ ${esc(W('try'))}</button>`;
      box.querySelector('button').addEventListener('click', () => openCalc(box), { once: true });
    });
  }

  // versions: the one in force today
  const today = new Date().toISOString().slice(0, 10);
  document.querySelectorAll('.versions li[data-from]').forEach((li) => {
    const from = li.dataset.from, to = li.dataset.to;
    if ((!from || from <= today) && (!to || today <= to)) li.classList.add('now');
  });

  // the map: lines from what a module builds on to the module; hovering one lights its own
  document.querySelectorAll('.map').forEach((map) => {
    const svg = map.querySelector('.wires');
    const nodes = new Map([...map.querySelectorAll('.node[data-node]')].map((n) => [n.dataset.node, n]));
    const draw = () => {
      const box = map.getBoundingClientRect();
      svg.innerHTML = '';
      nodes.forEach((n) => {
        for (const u of (n.dataset.uses || '').split(' ').filter(Boolean)) {
          const from = nodes.get(u); if (!from) continue;
          const a = from.getBoundingClientRect(), b = n.getBoundingClientRect();
          const x1 = a.right - box.left + map.scrollLeft, y1 = a.top + a.height / 2 - box.top;
          const x2 = b.left - box.left + map.scrollLeft, y2 = b.top + b.height / 2 - box.top;
          const p = document.createElementNS('http://www.w3.org/2000/svg', 'path');
          const dx = Math.max(24, (x2 - x1) / 2);
          p.setAttribute('d', `M${x1},${y1} C${x1 + dx},${y1} ${x2 - dx},${y2} ${x2},${y2}`);
          p.dataset.from = u; p.dataset.to = n.dataset.node;
          svg.appendChild(p);
        }
      });
    };
    const light = (id, on) => {
      svg.querySelectorAll('path').forEach((p) => {
        const hit = on && (p.dataset.from === id || p.dataset.to === id);
        p.classList.toggle('lit', hit);
        if (hit) { nodes.get(p.dataset.from).classList.add('lit'); nodes.get(p.dataset.to).classList.add('lit'); }
      });
      if (!on) nodes.forEach((n) => n.classList.remove('lit'));
    };
    nodes.forEach((n, id) => {
      n.addEventListener('mouseenter', () => light(id, true));
      n.addEventListener('mouseleave', () => light(id, false));
    });
    draw();
    window.addEventListener('resize', draw);
    if (document.fonts && document.fonts.ready) document.fonts.ready.then(draw);
  });

  const resume = document.getElementById('resume');
  if (resume) {
    const paths = Object.entries(store.get('paths', {})).sort((a, b) => b[1].at - a[1].at)
      .filter(([p]) => document.querySelector(`[data-progress="${CSS.escape(p)}"]`));
    if (paths.length) {
      const [p, info] = paths[0];
      const tile = document.querySelector(`.tile[data-progress="${CSS.escape(p)}"]`);
      const n = tile ? Number(tile.dataset.steps) : 0;
      if (done(p).length < n) {
        resume.innerHTML = `<p class="section-title">${esc(W('resume'))}</p><div class="grid"><a class="tile" href="${esc(info.href)}" data-progress="${esc(p)}" data-steps="${n}"><span class="chip kind-tag">${esc(W('path'))}</span><h3>${esc(tile ? tile.querySelector('h3').textContent : info.label)}</h3><div class="bar"><span></span></div><div class="stat"><span class="count"></span></div></a></div>`;
        paint(p, n);
      }
    }
  }
})();
"#;
