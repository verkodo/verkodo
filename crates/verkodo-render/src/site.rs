// SPDX-License-Identifier: AGPL-3.0-or-later
//! The knowledge as a site to read in: a home page, one article per module, one page per path to work through, a tree
//! of all modules, a search over every declaration, and links both ways – what a declaration builds on and where it is
//! used. A site (`write_site`) links its style, script and data as files under `assets/`; a single page (the preview of
//! the web editor) carries them inline, in exactly one `<style>` and one `<script>` – the editor adds its nonce to
//! those – and its overview holds every article, so that every link has somewhere to go.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use verkodo_core::proof::{Key, Status};
use verkodo_core::{Analysis, Loc, Sources, SymKind, Target, Ty};
use verkodo_syntax::ast::Item;

use super::R;
use crate::assets;
use crate::format::esc;

/// Where the pieces of a site go: one article per module, one page per path.
pub(crate) struct Layout {
    pub(crate) modules: Vec<Module>,
    /// For every file, its module (an index into `modules`), if it has one.
    module_of_file: Vec<Option<usize>>,
    /// For every item, the items it builds on and the items that use it.
    uses: Vec<BTreeSet<usize>>,
    used_by: Vec<BTreeSet<usize>>,
    /// For every symbol, where a link to it leads.
    pub(crate) hrefs: Vec<String>,
    /// For every module, the modules it builds on.
    module_uses: Vec<BTreeSet<usize>>,
}

pub(crate) struct Module {
    pub(crate) file: usize,
    pub(crate) id: String,
    pub(crate) title: String,
    /// A line under the title (the main declaration, when the title is a section number).
    pub(crate) lead: Option<String>,
    pub(crate) area: String,
    /// The groups between the area and the module, by their display names.
    pub(crate) groups: Vec<String>,
    /// The items of the module, the main one first, the others in their order; paths apart.
    pub(crate) items: Vec<usize>,
    pub(crate) paths: Vec<usize>,
}

impl Layout {
    /// `inline`: links for a single page, where everything but paths lies on the overview.
    pub(crate) fn new(ws: &Sources, a: &Analysis, lang: &str, inline: bool) -> Layout {
        let owner: Vec<usize> = (0..a.symbols.len())
            .map(|mut s| {
                while !matches!(a.symbols[s].loc, Loc::Item { .. }) {
                    match a.parent(s) {
                        Some(p) => s = p,
                        None => break,
                    }
                }
                s
            })
            .collect();
        let mut uses = vec![BTreeSet::new(); a.symbols.len()];
        let mut used_by = vec![BTreeSet::new(); a.symbols.len()];
        for (s, deps) in a.deps.iter().enumerate() {
            for &d in deps {
                let (from, to) = (owner[s], owner[d]);
                if from != to {
                    uses[from].insert(to);
                    used_by[to].insert(from);
                }
            }
        }
        // display names of the groups: a word of the sources or labels in the group that is the folder's name in
        // another case ("ustg" → "UStG"), else the folder's name
        let mut words: BTreeMap<String, String> = BTreeMap::new();
        for (i, _) in a.symbols.iter().enumerate() {
            let mut texts: Vec<String> = a.label(ws, i, lang).into_iter().collect();
            if let Loc::Item { file, item } = a.symbols[i].loc {
                texts.extend(item_sources(&ws.asts[file].items[item]));
            }
            for t in texts {
                for w in t.split(|c: char| !c.is_alphanumeric()) {
                    if w.len() > 1 && w.chars().any(char::is_uppercase) {
                        words.entry(w.to_lowercase()).or_insert_with(|| w.to_string());
                    }
                }
            }
        }
        // a folder named like a value of an enumeration ("bb", a Land) takes its label ("Brandenburg")
        let mut values: BTreeMap<String, String> = BTreeMap::new();
        for (i, s) in a.symbols.iter().enumerate() {
            if s.kind == SymKind::Value
                && let (Some(last), Some(label)) = (s.id.rsplit('.').next(), a.label(ws, i, lang))
            {
                values.entry(last.to_string()).or_insert(label);
            }
        }
        let display =
            |seg: &str| words.get(seg).or_else(|| values.get(seg)).cloned().unwrap_or_else(|| seg.to_string());
        let mut modules = Vec::new();
        let mut module_of_file = vec![None; ws.asts.len()];
        for (file, ast) in ws.asts.iter().enumerate() {
            let Some(m) = &ast.module else { continue };
            if ast.translation.is_some() {
                continue;
            }
            let id = m.joined();
            let segs: Vec<&str> = id.split('.').collect();
            let items: Vec<usize> = (0..a.symbols.len())
                .filter(|&s| {
                    a.symbols[s].loc.file() == file
                        && matches!(a.symbols[s].loc, Loc::Item { .. })
                        && a.symbols[s].kind != SymKind::Path
                })
                .collect();
            let paths: Vec<usize> = (0..a.symbols.len())
                .filter(|&s| a.symbols[s].loc.file() == file && a.symbols[s].kind == SymKind::Path)
                .collect();
            let label = |s: usize| a.label(ws, s, lang);
            // the main declaration: the current norm with the most parts (one with a date of validity first), else a path,
            // a concept, anything
            let members = |s: usize| match a.symbols[s].loc {
                Loc::Item { file, item } => match &ws.asts[file].items[item] {
                    Item::Norm(n) if !a.replaced_by.contains_key(&s) => Some((n.valid_from.is_some(), n.members.len())),
                    _ => None,
                },
                _ => None,
            };
            let main_norm = items
                .iter()
                .copied()
                .filter_map(|s| members(s).map(|n| (n, s)))
                .max_by_key(|&(n, s)| (n, std::cmp::Reverse(s)));
            let mut items = items;
            if let Some((_, s)) = main_norm {
                items.retain(|&i| i != s);
                items.insert(0, s);
            }
            let main = main_norm
                .and_then(|(_, s)| label(s))
                .or_else(|| paths.first().copied().and_then(label))
                .or_else(|| items.iter().copied().filter(|&s| a.symbols[s].kind == SymKind::Concept).find_map(label))
                .or_else(|| items.iter().copied().find_map(label));
            let last = segs.last().copied().unwrap_or_default();
            let group_names: Vec<String> =
                segs.get(1..segs.len().saturating_sub(1)).unwrap_or_default().iter().map(|s| display(s)).collect();
            let act = group_names.last().cloned().unwrap_or_default();
            let numbered = section_number(last).map(|n| if act.is_empty() { n } else { format!("{n} {act}") });
            let (title, lead) = match numbered {
                Some(n) => (n, main),
                None => (main.unwrap_or_else(|| id.clone()), None),
            };
            module_of_file[file] = Some(modules.len());
            modules.push(Module {
                file,
                id: id.clone(),
                title,
                lead,
                area: segs.first().copied().unwrap_or_default().to_string(),
                groups: group_names,
                items,
                paths,
            });
        }
        let hrefs = (0..a.symbols.len())
            .map(|s| {
                let id = &a.symbols[s].id;
                if a.symbols[s].kind == SymKind::Path {
                    format!("{id}.html")
                } else if inline {
                    format!("index.html#{id}")
                } else {
                    match module_of_file.get(a.symbols[s].loc.file()).copied().flatten() {
                        Some(m) => format!("{}.html#{id}", modules[m].id),
                        None => format!("index.html#{id}"),
                    }
                }
            })
            .collect();
        let mut module_uses = vec![BTreeSet::new(); modules.len()];
        for (s, targets) in uses.iter().enumerate() {
            let from = module_of_file.get(a.symbols[s].loc.file()).copied().flatten();
            for &t in targets {
                let to = module_of_file.get(a.symbols[t].loc.file()).copied().flatten();
                if let (Some(f), Some(t)) = (from, to)
                    && f != t
                {
                    module_uses[f].insert(t);
                }
            }
        }
        Layout { modules, module_of_file, uses, used_by, hrefs, module_uses }
    }

    fn module_of(&self, a: &Analysis, sym: usize) -> Option<&Module> {
        self.module_of_file.get(a.symbols[sym].loc.file()).copied().flatten().map(|m| &self.modules[m])
    }
}

fn item_sources(item: &Item) -> Vec<String> {
    item.meta().sources.iter().map(|(s, _)| s.clone()).collect()
}

/// "p19" → "§ 19", "p32a" → "§ 32a", "a2" → "Art. 2".
fn section_number(seg: &str) -> Option<String> {
    let num = |rest: &str| {
        rest.chars().next().is_some_and(|c| c.is_ascii_digit())
            && rest.chars().all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
    };
    if let Some(rest) = seg.strip_prefix('p').filter(|r| num(r)) {
        Some(format!("§ {rest}"))
    } else {
        seg.strip_prefix('a').filter(|r| num(r)).map(|rest| format!("Art. {rest}"))
    }
}

/// How Lean judged the parts of a module: checked, open, rejected.
#[derive(Default)]
struct Verdicts {
    checked: usize,
    open: usize,
    failed: usize,
}

impl<'a> R<'a> {
    fn words_json(&self) -> String {
        let keys = [
            "concept",
            "variant",
            "theorem",
            "example",
            "remark",
            "norm",
            "input",
            "judgement",
            "constant",
            "rule",
            "path",
            "enum",
            "value",
            "unit",
            "record",
            "field",
            "go",
            "nothing",
            "understood",
            "resume",
            "try",
            "compute",
            "results",
            "trace",
            "inputs_word",
            "loading",
            "calc_failed",
            "yes",
            "no",
            "not_given",
            "applied",
            "rule_default",
            "close",
            "unless",
        ];
        let pairs: Vec<String> = keys.iter().map(|k| format!("\"{k}\":\"{}\"", json_esc(self.w(k)))).collect();
        format!("{{{}}}", pairs.join(","))
    }

    /// What a field of a calculator asks for, as JSON: name, label, kind, and the values of an enumeration, the unit
    /// of an amount or the fields of a record.
    fn input_spec(&self, sym: usize, depth: usize) -> String {
        let ty = self.a.member_types.get(&sym).copied().unwrap_or(Ty::Unknown);
        let name = self.a.symbols[sym].id.rsplit('.').next().unwrap_or_default();
        let children = |of: usize, kind: SymKind| -> Vec<usize> {
            (0..self.a.symbols.len())
                .filter(|&c| self.a.symbols[c].kind == kind && self.a.parent(c) == Some(of))
                .collect()
        };
        let mut extra = String::new();
        match ty {
            Ty::Enum(e) => {
                let values: Vec<String> = children(e, SymKind::Value)
                    .into_iter()
                    .map(|v| {
                        let n = self.a.symbols[v].id.rsplit('.').next().unwrap_or_default();
                        format!("[\"{}\",\"{}\"]", json_esc(n), json_esc(&self.label(v)))
                    })
                    .collect();
                let _ = write!(extra, ",\"v\":[{}]", values.join(","));
            }
            Ty::Quantity(u) => {
                let _ = write!(extra, ",\"u\":\"{}\"", json_esc(&self.label(u)));
            }
            Ty::Record(r) | Ty::List(r) if depth < 3 => {
                let fields: Vec<String> =
                    children(r, SymKind::Field).into_iter().map(|f| self.input_spec(f, depth + 1)).collect();
                let _ = write!(extra, ",\"f\":[{}]", fields.join(","));
            }
            _ => {}
        }
        format!(
            "{{\"n\":\"{}\",\"l\":\"{}\",\"k\":\"{}\",\"j\":{}{extra}}}",
            json_esc(name),
            json_esc(&self.label(sym)),
            ty.name(),
            self.a.symbols[sym].kind == SymKind::Judgement
        )
    }

    /// The norms that can be tried, as JSON: for each, its inputs and its results – the rules and values no other
    /// member of the norm uses.
    fn forms_json(&self) -> String {
        let mut out = Vec::new();
        for n in 0..self.a.symbols.len() {
            if self.a.symbols[n].kind != SymKind::Norm {
                continue;
            }
            let mut members: Vec<usize> = (0..self.a.symbols.len()).filter(|&m| self.a.parent(m) == Some(n)).collect();
            members.sort_by_key(|&m| self.a.symbols[m].span.start);
            let inputs: Vec<usize> = members
                .iter()
                .copied()
                .filter(|&m| matches!(self.a.symbols[m].kind, SymKind::Input | SymKind::Judgement))
                .collect();
            if inputs.is_empty() {
                continue;
            }
            let used: BTreeSet<usize> = members.iter().flat_map(|&m| self.a.deps[m].iter().copied()).collect();
            let results: Vec<String> = members
                .iter()
                .copied()
                .filter(|&m| matches!(self.a.symbols[m].kind, SymKind::Rule | SymKind::Constant) && !used.contains(&m))
                .map(|m| format!("\"{}\"", json_esc(&self.a.symbols[m].id)))
                .collect();
            let specs: Vec<String> = inputs.iter().map(|&i| self.input_spec(i, 0)).collect();
            // the kind of every member, so that the page can write its value: money with "€", numbers grouped
            let types: Vec<String> = members
                .iter()
                .map(|&m| {
                    let ty = self.a.member_types.get(&m).copied().unwrap_or(Ty::Unknown);
                    format!("\"{}\":\"{}\"", json_esc(&self.a.symbols[m].id), ty.name())
                })
                .collect();
            out.push(format!(
                "\"{}\":{{\"inputs\":[{}],\"results\":[{}],\"types\":{{{}}}}}",
                json_esc(&self.a.symbols[n].id),
                specs.join(","),
                results.join(","),
                types.join(",")
            ));
        }
        format!("{{{}}}", out.join(","))
    }

    /// The sources for the calculator in the browser: the files under neutral names (their modules name them), so that
    /// no path of the machine that rendered them goes out.
    pub(crate) fn sources_js(&self) -> String {
        let files: Vec<String> = self
            .ws
            .files
            .iter()
            .enumerate()
            .map(|(i, f)| format!("{{\"path\":\"{i}.vk\",\"text\":\"{}\"}}", json_esc(&f.text)))
            .collect();
        format!("window.VK_SOURCES={{\"lang\":\"{}\",\"files\":[{}]}};", json_esc(self.lang), files.join(","))
    }

    /// `window.VK`: the cards, the search, the norms to try (`calc`: whether the calculator is there) and the words
    /// of the script.
    pub(crate) fn data_js(&self, l: &Layout, calc: bool) -> String {
        let mut out = String::from("window.VK={cards:{");
        for s in 0..self.a.symbols.len() {
            let _ = write!(out, "\"{}\":\"{}\",", json_esc(&self.a.symbols[s].id), json_esc(&self.card(s)));
        }
        out.push_str("},search:[");
        for s in 0..self.a.symbols.len() {
            let sym = &self.a.symbols[s];
            let kind = match self.modality(s) {
                Some(_) => "rule",
                None => sym.kind.keyword(),
            };
            let place = l.module_of(self.a, s).map(|m| m.title.clone()).unwrap_or_default();
            let _ = write!(
                out,
                "[\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"],",
                json_esc(&sym.id),
                json_esc(&self.title(s)),
                kind,
                json_esc(&l.hrefs[s]),
                json_esc(&place)
            );
        }
        let _ = write!(out, "],calc:{calc},forms:{},words:{}}};", self.forms_json(), self.words_json());
        out
    }

    fn verdicts(&self, m: &Module) -> Option<Verdicts> {
        let r = self.report?;
        let mut v = Verdicts::default();
        for (s, sym) in self.a.symbols.iter().enumerate() {
            if sym.loc.file() != m.file {
                continue;
            }
            let key = match sym.kind {
                SymKind::Theorem | SymKind::Example => Key::Theorem(s),
                SymKind::Variant => Key::Variant(s),
                SymKind::Norm => Key::Norm(s),
                SymKind::Rule | SymKind::Constant => Key::Member(s),
                _ => continue,
            };
            match r.get(key) {
                Some(Status::Checked) => v.checked += 1,
                Some(Status::Open) => v.open += 1,
                Some(Status::Failed(_)) => v.failed += 1,
                None => {}
            }
        }
        Some(v)
    }

    fn verdict_chips(&self, m: &Module) -> String {
        let Some(v) = self.verdicts(m) else { return String::new() };
        let mut out = String::new();
        if v.checked > 0 {
            let _ = write!(out, r#"<span class="chip ok">✓ {} {}</span>"#, v.checked, esc(self.w("n_checked")));
        }
        if v.open > 0 {
            let _ = write!(out, r#"<span class="chip open">{} {}</span>"#, v.open, esc(self.w("n_open")));
        }
        if v.failed > 0 {
            let _ = write!(out, r#"<span class="chip open">{} {}</span>"#, v.failed, esc(self.w("n_rejected")));
        }
        out
    }

    fn area_name(&self, area: &str) -> String {
        match area {
            "math" => self.w("area_math").into(),
            "de" => self.w("area_de").into(),
            "eu" => self.w("area_eu").into(),
            "calendar" => self.w("area_calendar").into(),
            other => other.to_string(),
        }
    }

    // ---- the frame of every page ----

    fn tree(&self, l: &Layout, here: Option<&str>) -> String {
        // area → groups → modules, in the order of their ids
        let mut areas: BTreeMap<&str, BTreeMap<Vec<String>, Vec<&Module>>> = BTreeMap::new();
        for m in &l.modules {
            let key = m.groups.iter().map(|g| g.to_lowercase()).collect();
            areas.entry(m.area.as_str()).or_default().entry(key).or_default().push(m);
        }
        let mut out = String::from(r#"<ul class="tree">"#);
        for (area, groups) in &areas {
            let open = groups.values().flatten().any(|m| Some(m.id.as_str()) == here) || here == Some("index");
            let _ = write!(
                out,
                r#"<li><details{}><summary>{}</summary><ul>"#,
                if open { " open" } else { "" },
                esc(&self.area_name(area))
            );
            for mods in groups.values() {
                let groups = &mods[0].groups;
                let leaves: String = mods
                    .iter()
                    .map(|m| {
                        let cls = if Some(m.id.as_str()) == here { " here" } else { "" };
                        let name = match &m.lead {
                            Some(lead) => format!("{} · {lead}", m.title),
                            None => m.title.clone(),
                        };
                        format!(
                            r#"<li><a class="{cls}" href="{}.html" data-page="{}" title="{}"><span>{}</span><span class="seen">●</span></a></li>"#,
                            esc(&m.id),
                            esc(&m.id),
                            esc(&m.id),
                            esc(&name)
                        )
                    })
                    .collect();
                if groups.is_empty() {
                    out.push_str(&leaves);
                } else {
                    let open = mods.iter().any(|m| Some(m.id.as_str()) == here);
                    let _ = write!(
                        out,
                        r#"<li class="group"><details{}><summary>{}</summary><ul>{leaves}</ul></details></li>"#,
                        if open { " open" } else { "" },
                        esc(&groups.join(" › "))
                    );
                }
            }
            out.push_str("</ul></details></li>");
        }
        out.push_str("</ul>");
        out
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn frame(
        &self,
        l: &Layout,
        inline: bool,
        site_title: &str,
        title: &str,
        page: &str,
        body_attrs: &str,
        main: &str,
        main_class: &str,
        rail: &str,
    ) -> String {
        let head = if inline {
            format!("<style>{}</style>", assets::CSS)
        } else {
            r#"<link rel="stylesheet" href="assets/site.css"><link rel="icon" href="assets/verkodo-mark-on-light.svg"><script>try{var t=JSON.parse(localStorage.getItem('verkodo:theme'));if(t==='light'||t==='dark')document.documentElement.dataset.theme=t}catch(e){}</script>"#.to_string()
        };
        let brand = if inline {
            format!("<span>{}</span>", esc(site_title))
        } else {
            format!(
                r#"<img class="on-light" src="assets/verkodo-mark-on-light.svg" alt=""><img class="on-dark" src="assets/verkodo-mark-on-dark.svg" alt=""><span>{}</span>"#,
                esc(site_title)
            )
        };
        let side = if inline {
            String::new()
        } else {
            format!(r#"<nav class="side" id="side">{}</nav>"#, self.tree(l, Some(page)))
        };
        let layout = match (inline, rail.is_empty()) {
            (true, _) => r#"<div class="layout wide" style="grid-template-columns:minmax(0,1fr)">"#.to_string(),
            (false, true) => r#"<div class="layout wide">"#.to_string(),
            (false, false) => r#"<div class="layout">"#.to_string(),
        };
        let rail =
            if rail.is_empty() || inline { String::new() } else { format!(r#"<aside class="rail">{rail}</aside>"#) };
        let script = if inline {
            format!("<script>{}{}</script>", self.data_js(l, false), assets::JS)
        } else {
            r#"<script src="assets/data.js"></script><script src="assets/site.js"></script>"#.to_string()
        };
        let menu = if inline { "" } else { r#"<button class="icon" id="menu" aria-label="Menu">☰</button>"# };
        format!(
            r#"<!doctype html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
{head}
</head>
<body data-page="{page}" {body_attrs}>
<header class="top">{menu}<a class="brand" href="index.html">{brand}</a><div class="search"><input type="search" id="q" placeholder="{search}" autocomplete="off" aria-label="{search}"><div class="results" id="results" hidden></div></div><button class="icon" id="theme" aria-label="{theme}" title="{theme}">◐</button></header>
{layout}{side}<main class="content {main_class}">{main}</main>{rail}</div>
{script}
</body>
</html>
"#,
            lang = esc(self.lang),
            title = esc(&if title == site_title { title.to_string() } else { format!("{title} – {site_title}") }),
            page = esc(page),
            search = esc(self.w("search")),
            theme = esc(self.w("theme")),
        )
    }

    // ---- the pages ----

    fn path_tile(&self, l: &Layout, p: usize) -> String {
        let steps = self.path_steps(p).len();
        let place = l.module_of(self.a, p).map(|m| m.title.clone()).unwrap_or_default();
        format!(
            r#"<a class="tile" href="{href}" data-progress="{id}" data-steps="{steps}"><span class="chip kind-tag">{kind}</span><h3>{label}</h3><p>{steps} {steps_word} · {place}</p><div class="bar"><span></span></div><div class="stat"><span class="count"></span></div></a>"#,
            href = esc(&l.hrefs[p]),
            id = esc(&self.a.symbols[p].id),
            kind = esc(self.w("path")),
            label = esc(&self.label(p)),
            steps_word = esc(self.w("steps")),
            place = esc(&place),
        )
    }

    pub(crate) fn home(&self, l: &Layout, inline: bool, site_title: &str) -> String {
        let mut main = format!(
            r#"<section class="hero"><h1>{}</h1><p>{}</p></section><div id="resume"></div>"#,
            esc(site_title),
            esc(self.w("tagline"))
        );
        let paths: Vec<usize> = l.modules.iter().flat_map(|m| m.paths.iter().copied()).collect();
        if !paths.is_empty() {
            let _ = write!(main, r#"<p class="section-title">{}</p><div class="grid">"#, esc(self.w("paths")));
            for p in paths {
                main.push_str(&self.path_tile(l, p));
            }
            main.push_str("</div>");
        }
        let mut areas: BTreeMap<&str, Vec<&Module>> = BTreeMap::new();
        for m in &l.modules {
            areas.entry(m.area.as_str()).or_default().push(m);
        }
        let _ = write!(main, r#"<p class="section-title">{}</p><div class="grid">"#, esc(self.w("areas")));
        for (area, mods) in &areas {
            let n: usize = mods.iter().map(|m| m.items.len()).sum();
            let _ = write!(
                main,
                r#"<div class="tile"><h3>{}</h3><div class="stat"><span>{} {}</span><span>{} {}</span></div><ul>"#,
                esc(&self.area_name(area)),
                mods.len(),
                esc(self.w("articles")),
                n,
                esc(self.w("entries"))
            );
            for m in mods {
                let href = if inline { format!("#m-{}", m.id) } else { format!("{}.html", m.id) };
                let name = match &m.lead {
                    Some(lead) => format!("{} · {lead}", m.title),
                    None => m.title.clone(),
                };
                let _ = write!(main, r#"<li><a href="{}">{}</a></li>"#, esc(&href), esc(&name));
            }
            main.push_str("</ul></div>");
        }
        main.push_str("</div>");
        if l.module_uses.iter().any(|u| !u.is_empty()) {
            let _ = write!(main, r#"<p class="section-title">{}</p>{}"#, esc(self.w("map")), self.map(l, inline));
        }
        let _ = write!(main, r#"<p class="hint">{}</p>"#, esc(self.w("hint")));
        if inline {
            for m in &l.modules {
                let _ = write!(main, r#"<hr id="m-{}">{}"#, esc(&m.id), self.article_main(l, m, true));
            }
        }
        self.frame(l, inline, site_title, site_title, "index", "", &main, "home", "")
    }

    fn links_row(&self, l: &Layout, key: &str, set: &BTreeSet<usize>) -> String {
        if set.is_empty() {
            return String::new();
        }
        let mut items: Vec<usize> = set.iter().copied().collect();
        items.sort_by_key(|&s| self.title(s).to_lowercase());
        let shown = 14;
        let chips: String = items
            .iter()
            .take(shown)
            .map(|&s| {
                format!(
                    r#"<a class="chip" href="{}" data-ref="{}">{}</a>"#,
                    esc(&l.hrefs[s]),
                    esc(&self.a.symbols[s].id),
                    esc(&self.title(s))
                )
            })
            .collect();
        let more = if items.len() > shown {
            format!(r#"<span class="hint">+{}</span>"#, items.len() - shown)
        } else {
            String::new()
        };
        format!(r#"<div class="links"><span class="key">{}</span>{chips}{more}</div>"#, esc(self.w(key)))
    }

    /// The parts of an item shown inside its section: the variants of a concept, the members of a norm, the values of
    /// an enumeration, the fields of a record – each with its own anchor.
    fn parts(&self, item: usize) -> String {
        let id = &self.a.symbols[item].id;
        let mut parts: Vec<usize> = (0..self.a.symbols.len())
            .filter(|&s| s != item && self.a.parent(s) == Some(item) && self.a.symbols[s].id.starts_with(id.as_str()))
            .collect();
        parts.sort_by_key(|&s| self.a.symbols[s].span.start);
        if parts.is_empty() {
            return String::new();
        }
        let open = self.a.symbols[item].kind == SymKind::Concept;
        let mut out = String::from(r#"<div class="parts">"#);
        for s in parts {
            let body = self.body(s);
            let _ = write!(
                out,
                r#"<details class="part" id="{id}"{open}><summary><span class="kind">{kind}</span><span>{title}</span><span class="id">{id}</span></summary><div class="body">{body}</div></details>"#,
                id = esc(&self.a.symbols[s].id),
                open = if open { " open" } else { "" },
                kind = esc(self.kind_word(s)),
                title = esc(&self.label(s)),
            );
        }
        out.push_str("</div>");
        out
    }

    fn article_main(&self, l: &Layout, m: &Module, inline: bool) -> String {
        let mut main = String::new();
        let crumbs: Vec<String> =
            std::iter::once(self.area_name(&m.area)).chain(m.groups.iter().cloned()).map(|c| esc(&c)).collect();
        let home =
            if inline { String::new() } else { format!(r#"<a href="index.html">{}</a> › "#, esc(self.w("home"))) };
        let _ = write!(main, r#"<div class="crumbs">{home}{}</div><h1>{}</h1>"#, crumbs.join(" › "), esc(&m.title));
        if let Some(lead) = &m.lead {
            let _ = write!(main, r#"<p class="lead">{}</p>"#, esc(lead));
        }
        let _ = write!(
            main,
            r#"<div class="meta"><span class="chip">{} {}</span>{}<span class="chip mono">{}</span></div>"#,
            m.items.len(),
            esc(self.w("entries")),
            self.verdict_chips(m),
            esc(&m.id)
        );
        if !m.paths.is_empty() {
            main.push_str(r#"<div class="grid">"#);
            for &p in &m.paths {
                main.push_str(&self.path_tile(l, p));
            }
            main.push_str("</div>");
        }
        for &s in &m.items {
            let _ = write!(
                main,
                r#"<section class="decl" id="{id}"><header><span class="chip kind-tag">{kind}</span><h2>{title}</h2><span class="id">{id}</span></header>{body}{calc}{parts}{uses}{used}</section>"#,
                id = esc(&self.a.symbols[s].id),
                kind = esc(self.kind_word(s)),
                title = esc(&self.title(s)),
                body = self.body_with(s, true),
                calc = if self.a.symbols[s].kind == SymKind::Norm {
                    self.versions(s)
                        + &format!(r#"<div class="calc" data-norm="{}" hidden></div>"#, esc(&self.a.symbols[s].id))
                } else {
                    String::new()
                },
                parts = self.parts(s),
                uses = self.links_row(l, "builds_on", &l.uses[s]),
                used = self.links_row(l, "used_by", &l.used_by[s]),
            );
        }
        main
    }

    /// The versions of a provision as a time line – the one shown marked, the one in force today found by the script –
    /// and what changed against the version before: parts added, dropped, or written differently.
    fn versions(&self, norm: usize) -> String {
        let chain = self.a.versions(norm);
        if chain.len() < 2 {
            return String::new();
        }
        let norm_of = |v: usize| match self.a.symbols[v].loc {
            Loc::Item { file, item } => match &self.ws.asts[file].items[item] {
                Item::Norm(n) => Some((file, n)),
                _ => None,
            },
            _ => None,
        };
        let mut out = format!(r#"<div class="versions"><span class="key">{}</span><ol>"#, esc(self.w("versions")));
        for &v in &chain {
            let Some((_, n)) = norm_of(v) else { continue };
            let from = n.valid_from.as_ref().map(|(d, _)| d.as_str()).unwrap_or_default();
            let to = n.valid_to.as_ref().map(|(d, _)| d.as_str()).unwrap_or_default();
            let span = match (from.is_empty(), to.is_empty()) {
                (false, false) => format!("{from} – {to}"),
                (false, true) => format!("{} {from}", self.w("valid_from")),
                _ => String::new(),
            };
            let _ = write!(
                out,
                r#"<li class="{here}" data-from="{from}" data-to="{to}"><a href="{href}" data-ref="{id}">{label}</a><span class="when">{span}</span><span class="now-tag">{now}</span></li>"#,
                here = if v == norm { "here" } else { "" },
                href = esc(&self.f_href(v)),
                id = esc(&self.a.symbols[v].id),
                label = esc(&self.label(v)),
                span = esc(&span),
                now = esc(self.w("in_force")),
            );
        }
        out.push_str("</ol>");
        // what changed against the version before this one
        if let (Some(&old), Some((nf, new))) = (self.a.replaces.get(&norm), norm_of(norm))
            && let Some((of, oldn)) = norm_of(old)
        {
            let text = |file: usize, span: verkodo_syntax::Span| -> String {
                let t = &self.ws.files[file].text;
                t.get(span.start..span.end).unwrap_or_default().split_whitespace().collect::<Vec<_>>().join(" ")
            };
            let what = |file: usize, m: &verkodo_syntax::ast::Member| -> String {
                use verkodo_syntax::ast::MemberKind;
                match &m.kind {
                    MemberKind::Constant(e) => text(file, e.span),
                    MemberKind::Rule { default, unless } => {
                        let mut s = default.as_ref().map(|e| text(file, e.span)).unwrap_or_default();
                        for u in unless {
                            s.push_str(" | ");
                            s.push_str(&text(file, u.span));
                        }
                        s
                    }
                    _ => text(file, m.ty.name.span),
                }
            };
            let id_of =
                |norm: usize, name: &str| self.a.by_id.get(&format!("{}.{name}", self.a.symbols[norm].id)).copied();
            let mut rows = Vec::new();
            for m in &new.members {
                let label = id_of(norm, &m.name.text).map(|s| self.label(s)).unwrap_or_else(|| m.name.text.clone());
                match oldn.members.iter().find(|o| o.name.text == m.name.text) {
                    None => rows.push(format!(r#"<li><span class="chip ok">+</span> {}</li>"#, esc(&label))),
                    Some(o) if what(of, o) != what(nf, m) => {
                        let value = |file: usize, mm: &verkodo_syntax::ast::Member| match &mm.kind {
                            verkodo_syntax::ast::MemberKind::Constant(e) => Some(self.expr(file, e)),
                            _ => None,
                        };
                        let change = match (value(of, o), value(nf, m)) {
                            (Some(a), Some(b)) => format!(": {a} → {b}"),
                            _ => String::new(),
                        };
                        rows.push(format!(r#"<li><span class="chip">≠</span> {}{change}</li>"#, esc(&label)));
                    }
                    Some(_) => {}
                }
            }
            for o in &oldn.members {
                if !new.members.iter().any(|m| m.name.text == o.name.text) {
                    let label = id_of(old, &o.name.text).map(|s| self.label(s)).unwrap_or_else(|| o.name.text.clone());
                    rows.push(format!(r#"<li><span class="chip open">−</span> {}</li>"#, esc(&label)));
                }
            }
            if !rows.is_empty() {
                let _ = write!(
                    out,
                    r#"<details class="diff"><summary>{} {}</summary><ul>{}</ul></details>"#,
                    esc(self.w("changed_against")),
                    esc(&self.label(old)),
                    rows.join("")
                );
            }
        }
        out.push_str("</div>");
        out
    }

    /// Where a link to a symbol leads, as the format writes it.
    fn f_href(&self, sym: usize) -> String {
        let link = self.f.sym(sym, &self.a.symbols[sym].id, "");
        link.split("href=\"")
            .nth(1)
            .and_then(|r| r.split('"').next())
            .map(|h| h.replace("&amp;", "&"))
            .unwrap_or_default()
    }

    /// A module among the others: what it builds on above, what builds on it below.
    fn neighbours(&self, l: &Layout, m: usize) -> String {
        let up: Vec<usize> = l.module_uses[m].iter().copied().collect();
        let down: Vec<usize> = (0..l.modules.len()).filter(|&o| l.module_uses[o].contains(&m)).collect();
        if up.is_empty() && down.is_empty() {
            return String::new();
        }
        let link = |i: usize| {
            format!(r#"<a class="node" href="{}.html">{}</a>"#, esc(&l.modules[i].id), esc(&l.modules[i].title))
        };
        let mut out = format!(r#"<h4>{}</h4><div class="hood">"#, esc(self.w("context")));
        if !up.is_empty() {
            let _ = write!(
                out,
                r#"<div class="tier">{}</div><div class="wire">↓</div>"#,
                up.into_iter().map(link).collect::<String>()
            );
        }
        let _ = write!(out, r#"<div class="tier"><span class="node here">{}</span></div>"#, esc(&l.modules[m].title));
        if !down.is_empty() {
            let _ = write!(
                out,
                r#"<div class="wire">↓</div><div class="tier">{}</div>"#,
                down.into_iter().map(link).collect::<String>()
            );
        }
        out.push_str("</div>");
        out
    }

    /// Every module in columns by how deep it builds on others – the foundations left – with lines drawn by the
    /// script from what each builds on.
    fn map(&self, l: &Layout, inline: bool) -> String {
        let n = l.modules.len();
        let mut depth = vec![0usize; n];
        for _ in 0..n {
            let mut changed = false;
            for i in 0..n {
                let d = l.module_uses[i].iter().map(|&u| depth[u] + 1).max().unwrap_or(0);
                if d > depth[i] && d < n {
                    depth[i] = d;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let cols = depth.iter().copied().max().unwrap_or(0) + 1;
        let mut out = format!(r#"<div class="map" style="--cols:{cols}"><svg class="wires" aria-hidden="true"></svg>"#);
        for c in 0..cols {
            out.push_str(r#"<div class="col">"#);
            let mut in_col: Vec<usize> = (0..n).filter(|&i| depth[i] == c).collect();
            in_col
                .sort_by(|&a, &b| (&l.modules[a].area, &l.modules[a].id).cmp(&(&l.modules[b].area, &l.modules[b].id)));
            for i in in_col {
                let m = &l.modules[i];
                let href = if inline { format!("#m-{}", m.id) } else { format!("{}.html", m.id) };
                let to: Vec<String> = l.module_uses[i].iter().map(|u| u.to_string()).collect();
                let _ = write!(
                    out,
                    r#"<a class="node" href="{}" data-node="{i}" data-uses="{}"><span class="area">{}</span>{}</a>"#,
                    esc(&href),
                    to.join(" "),
                    esc(&self.area_name(&m.area)),
                    esc(&m.title)
                );
            }
            out.push_str("</div>");
        }
        out.push_str("</div>");
        out
    }

    pub(crate) fn article(&self, l: &Layout, m: &Module, site_title: &str) -> String {
        let main = self.article_main(l, m, false);
        let mut rail = format!(r#"<h4>{}</h4><ol class="toc">"#, esc(self.w("on_page")));
        for &s in &m.items {
            let _ = write!(rail, r##"<li><a href="#{}">{}</a></li>"##, esc(&self.a.symbols[s].id), esc(&self.title(s)));
        }
        rail.push_str("</ol>");
        if let Some(i) = l.modules.iter().position(|x| x.id == m.id) {
            rail.push_str(&self.neighbours(l, i));
        }
        let used: BTreeSet<usize> = m
            .items
            .iter()
            .flat_map(|&s| l.used_by[s].iter().copied())
            .filter(|&u| self.a.symbols[u].loc.file() != m.file)
            .collect();
        if !used.is_empty() {
            let _ = write!(rail, "<h4>{}</h4><ul>", esc(self.w("used_by")));
            for u in used.into_iter().take(20) {
                let _ = write!(rail, r#"<li><a href="{}">{}</a></li>"#, esc(&l.hrefs[u]), esc(&self.title(u)));
            }
            rail.push_str("</ul>");
        }
        self.frame(l, false, site_title, &m.title, &m.id, "", &main, "", &rail)
    }

    pub(crate) fn path_steps(&self, path: usize) -> Vec<usize> {
        let Loc::Item { file, item } = self.a.symbols[path].loc else { return Vec::new() };
        let Item::Path(d) = &self.ws.asts[file].items[item] else { return Vec::new() };
        d.steps
            .iter()
            .filter_map(|st| match self.a.refs.get(&(file, st.span)) {
                Some(Target::Symbol(s)) => Some(*s),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn reading(&self, l: &Layout, path: usize, inline: bool, site_title: &str) -> String {
        let steps = self.path_steps(path);
        let title = self.label(path);
        let id = &self.a.symbols[path].id;
        let place = l.module_of(self.a, path);
        let mut main = String::new();
        if let Some(m) = place {
            let back = if inline { "index.html".to_string() } else { format!("{}.html", m.id) };
            let home = if inline {
                String::new()
            } else {
                format!(r#"<a href="index.html">{}</a> › "#, esc(self.w("home")))
            };
            let _ = write!(main, r#"<div class="crumbs">{home}<a href="{}">{}</a></div>"#, esc(&back), esc(&m.title));
        }
        let _ = write!(
            main,
            r#"<h1>{}</h1><div class="progress" data-progress="{}" data-steps="{}"><span class="chip kind-tag">{}</span><span>{} {}</span><div class="bar"><span></span></div><span class="count"></span></div><p class="hint">{}</p>"#,
            esc(&title),
            esc(id),
            steps.len(),
            esc(self.w("path")),
            steps.len(),
            esc(self.w("steps")),
            esc(self.w("hint"))
        );
        for (n, &s) in steps.iter().enumerate() {
            let next = if n + 1 < steps.len() {
                format!(r##"<a class="btn next" href="#step-{}">{} ↓</a>"##, n + 2, esc(self.w("next")))
            } else {
                String::new()
            };
            let _ = write!(
                main,
                r#"<section class="decl step" id="step-{num}"><header><span class="num">{num}</span><span class="chip kind-tag">{kind}</span><h2>{title}</h2></header>{body}<div class="step-foot"><button class="btn got" type="button" aria-pressed="false">✓ {got}</button><a class="btn" href="{href}" data-ref="{sid}">{context}</a>{next}</div></section>"#,
                num = n + 1,
                kind = esc(self.kind_word(s)),
                title = esc(&self.title(s)),
                body = self.body(s) + &self.in_path(s, path),
                got = esc(self.w("understood_btn")),
                href = esc(&l.hrefs[s]),
                sid = esc(&self.a.symbols[s].id),
                context = esc(self.w("in_context")),
            );
        }
        let _ = write!(main, r#"<div class="finished">∎ {}</div>"#, esc(self.w("finished")));
        let mut rail = format!(r#"<h4>{}</h4><ol class="toc steps-nav">"#, esc(self.w("steps")));
        for (n, &s) in steps.iter().enumerate() {
            let _ = write!(rail, r##"<li data-step="{n}"><a href="#step-{}">{}</a></li>"##, n + 1, esc(&self.title(s)));
        }
        rail.push_str("</ol>");
        let page_key = place.map(|m| m.id.clone()).unwrap_or_default();
        let attrs = format!(r#"data-path="{}""#, esc(id));
        self.frame(l, inline, site_title, &title, &page_key, &attrs, &main, "", &rail)
    }
}

/// A string for a JavaScript string literal in double quotes; `</` is broken so that it cannot end the script.
fn json_esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '<' => out.push_str("\\u003c"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> (Sources, Analysis) {
        let a = r#"module law.act.p1 original en
            norm tax { label en "The tax" valid from 2026-01-01
              input wage : money { label en "Wage" }
              rule due : money = wage * 0.1 { label en "Due" } }"#;
        let b = r#"module law.act.p2 original en
            norm relief { label en "Relief" valid from 2026-01-01
              input wage : money { label en "Wage" }
              rule left : money = wage - law.act.p1.tax.due(wage = wage) { label en "Left" } }
            path guide { label en "Guide" step relief step law.act.p1.tax }"#;
        let ws = Sources::from_sources(vec![("p1.vk".into(), a.into()), ("p2.vk".into(), b.into())]);
        let analysis = verkodo_core::analyze(&ws);
        assert!(analysis.diags.iter().all(|d| d.severity == verkodo_syntax::Severity::Warning), "{:?}", analysis.diags);
        (ws, analysis)
    }

    #[test]
    fn modules_get_titles_and_links_go_both_ways() {
        let (ws, a) = sample();
        let l = Layout::new(&ws, &a, "en", false);
        let titles: Vec<&str> = l.modules.iter().map(|m| m.title.as_str()).collect();
        assert_eq!(titles, ["§ 1 act", "§ 2 act"]);
        assert_eq!(l.modules[0].lead.as_deref(), Some("The tax"));
        let (tax, relief) = (a.by_id["law.act.p1.tax"], a.by_id["law.act.p2.relief"]);
        assert!(l.used_by[tax].contains(&relief) && l.uses[relief].contains(&tax));
        assert_eq!(l.hrefs[a.by_id["law.act.p1.tax.due"]], "law.act.p1.html#law.act.p1.tax.due");
        assert_eq!(l.hrefs[a.by_id["law.act.p2.guide"]], "law.act.p2.guide.html");
    }

    #[test]
    fn a_site_has_its_pages_and_a_single_page_carries_one_style_and_one_script() {
        let (ws, a) = sample();
        let dir = std::env::temp_dir().join(format!("verkodo-site-{}", std::process::id()));
        let written = crate::write_site(&ws, &a, None, &dir, "en", "Test", Some(b"\0asm")).unwrap();
        let names: Vec<String> =
            written.iter().map(|p| p.strip_prefix(&dir).unwrap().to_string_lossy().replace('\\', "/")).collect();
        for want in ["index.html", "law.act.p1.html", "law.act.p2.html", "law.act.p2.guide.html", "assets/data.js"] {
            assert!(names.iter().any(|n| n == want), "{want} missing in {names:?}");
        }
        let article = std::fs::read_to_string(dir.join("law.act.p1.html")).unwrap();
        assert!(article.contains(r#"id="law.act.p1.tax.due""#), "a part has its anchor");
        assert!(article.contains("Used in") && article.contains("law.act.p2.html#law.act.p2.relief"), "{article}");
        let path = std::fs::read_to_string(dir.join("law.act.p2.guide.html")).unwrap();
        assert!(path.contains(r#"data-path="law.act.p2.guide""#) && path.contains(r#"id="step-2""#));
        // the calculator: a norm with its inputs and results, the sources under neutral names
        let data = std::fs::read_to_string(dir.join("assets/data.js")).unwrap();
        assert!(data.contains(r#""law.act.p2.relief":{"inputs":[{"n":"wage","l":"Wage","k":"money""#), "{data}");
        assert!(data.contains(r#""results":["law.act.p2.relief.left"]"#) && data.contains("calc:true"));
        let sources = std::fs::read_to_string(dir.join("assets/sources.js")).unwrap();
        assert!(sources.contains(r#""path":"0.vk""#) && !sources.contains("p1.vk"), "{sources}");
        assert!(article.contains(r#"class="calc" data-norm="law.act.p1.tax""#));
        // the map: § 2 builds on § 1; the article of § 1 shows § 2 below it
        let home = std::fs::read_to_string(dir.join("index.html")).unwrap();
        assert!(home.contains(r#"data-node="1" data-uses="0""#), "{home}");
        assert!(article.contains(r#"class="hood""#) && article.contains(r#"<a class="node" href="law.act.p2.html">"#));
        std::fs::remove_dir_all(&dir).unwrap();
        let single = crate::index_html(&ws, &a, None, "en");
        assert_eq!(single.matches("<style").count(), 1);
        assert_eq!(single.matches("<script").count(), 1);
        assert!(single.contains(r#"id="law.act.p2.relief""#), "the overview holds every article");
    }
}
