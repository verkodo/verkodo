// SPDX-License-Identifier: AGPL-3.0-or-later
//! Knowledge written in verkodo: reading `.vk` files, checking them, letting Lean check them and writing HTML. The
//! I/O lives here; the domain in `verkodo-core` stays without it.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

pub use verkodo_core::proof::{DeclLocation, LeanName, Report, ReportData, Status};
pub use verkodo_core::query::{Found, at, goals_at};
pub use verkodo_core::{Analysis, EXTENSION, Sources, SymKind, analyze};
pub use verkodo_lean::ProveOutcome;
pub use verkodo_render::{goals_hover, lean_hover, symbol_hover};
pub use verkodo_syntax::format::{Options as FormatOptions, Style, format, format_with};
pub use verkodo_syntax::lexer::spaces_for_tabs;
pub use verkodo_syntax::{Diagnostic, Severity};

/// Every member of norm `norm` (its id) evaluated for the inputs given as text by name – inputs, constants and rules
/// with their values, exactly (fractions), as a person reads them; none if there is no such norm.
pub fn evaluate(
    sources: &Sources,
    analysis: &Analysis,
    norm: &str,
    inputs: &HashMap<String, String>,
) -> Option<Vec<(String, String, bool)>> {
    let &n = analysis.by_id.get(norm).filter(|&&n| analysis.symbols[n].kind == SymKind::Norm)?;
    Some(
        verkodo_core::eval::evaluate(sources, analysis, n, inputs)
            .into_iter()
            .map(|s| {
                let name = analysis.symbols[s.sym].id.rsplit('.').next().unwrap_or_default().to_string();
                let known = !matches!(s.value, verkodo_core::eval::Value::Unknown(_));
                (name, s.value.show(analysis), known)
            })
            .collect(),
    )
}

/// The label of a declaration in `lang` (else English, else the first), or the last part of its id.
pub fn label(sources: &Sources, analysis: &Analysis, sym: usize, lang: &str) -> String {
    analysis
        .label(sources, sym, lang)
        .unwrap_or_else(|| analysis.symbols[sym].id.rsplit('.').next().unwrap_or_default().to_string())
}

/// Reads the given files and every `.vk` file below the given directories, in a stable order, and parses them.
pub fn load(paths: &[PathBuf]) -> io::Result<Sources> {
    load_with(paths, &HashMap::new())
}

/// Like [`load`], but texts in `open` – files open in an editor, perhaps not saved – win over the disk, and open files
/// that are not found below `paths` count too.
pub fn load_with(paths: &[PathBuf], open: &HashMap<PathBuf, String>) -> io::Result<Sources> {
    let mut found = Vec::new();
    for p in paths {
        collect(p, &mut found)?;
    }
    found.extend(open.keys().cloned());
    found.sort();
    found.dedup();
    let mut sources = Vec::new();
    for p in found {
        let text = match open.get(&p) {
            Some(text) => text.clone(),
            None => std::fs::read_to_string(&p)?,
        };
        sources.push((p.display().to_string(), text));
    }
    Ok(Sources::from_sources(sources))
}

/// Directories that are never searched: hidden ones (`.git`, Lean's `.lake`), build output, packages.
fn skipped(dir: &Path) -> bool {
    dir.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with('.') || n == "target" || n == "node_modules")
}

fn collect(p: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    if p.is_dir() {
        for entry in std::fs::read_dir(p)? {
            let path = entry?.path();
            if path.is_dir() && !skipped(&path) || path.extension().is_some_and(|e| e == EXTENSION) {
                collect(&path, out)?;
            }
        }
    } else {
        out.push(p.to_path_buf());
    }
    Ok(())
}

/// Exports to Lean below `<lean_dir>/generated/` and lets `lake env lean` check every file.
pub fn prove(sources: &Sources, analysis: &Analysis, lean_dir: &Path) -> io::Result<ProveOutcome> {
    verkodo_lean::prove(sources, analysis, lean_dir)
}

/// Lets Lean check sources given as (path, text) – files from an editor, not from disk: parses and analyses them, and
/// if nothing stands in the way, exports and checks them in `lean_dir`.
pub fn prove_texts(texts: Vec<(String, String)>, lean_dir: &Path) -> io::Result<(Sources, ProveOutcome)> {
    let sources = Sources::from_sources(texts);
    let analysis = analyze(&sources);
    let outcome = prove(&sources, &analysis, lean_dir)?;
    Ok((sources, outcome))
}

/// Writes the HTML pages – an index and one page per path – into `out`; with Lean's verdict if there is a report.
pub fn render(
    sources: &Sources,
    analysis: &Analysis,
    report: Option<&Report>,
    out: &Path,
    lang: &str,
) -> io::Result<Vec<PathBuf>> {
    verkodo_render::write_site(sources, analysis, report, out, lang)
}

/// How `pdf` runs Typst.
#[derive(Debug, Clone)]
pub struct TypstOptions {
    /// The command, e.g. `typst` or a path to it.
    pub command: String,
    /// A directory with the IBM Plex fonts of the brand (`typst --font-path`); without it the fonts Typst brings.
    pub fonts: Option<PathBuf>,
}

/// The paths a selection names: all paths, or those whose id is `name` or ends in `.name`.
pub fn select_paths(analysis: &Analysis, name: Option<&str>) -> Vec<usize> {
    (0..analysis.symbols.len())
        .filter(|&i| analysis.symbols[i].kind == verkodo_core::SymKind::Path)
        .filter(|&i| {
            let id = &analysis.symbols[i].id;
            name.is_none_or(|n| id == n || id.ends_with(&format!(".{n}")))
        })
        .collect()
}

/// Writes each path as a Typst document (`<id>.typ`) into `out` and lets Typst make a PDF of it (`<id>.pdf`).
pub fn pdf(
    sources: &Sources,
    analysis: &Analysis,
    report: Option<&Report>,
    paths: &[usize],
    out: &Path,
    lang: &str,
    typst: &TypstOptions,
) -> io::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(out)?;
    let mut written = Vec::new();
    for &p in paths {
        let id = &analysis.symbols[p].id;
        let doc = verkodo_render::path_document(sources, analysis, report, p, lang, typst.fonts.is_some());
        let typ = out.join(format!("{id}.typ"));
        let pdf = out.join(format!("{id}.pdf"));
        std::fs::write(&typ, doc)?;
        let mut cmd = std::process::Command::new(&typst.command);
        cmd.arg("compile");
        if let Some(fonts) = &typst.fonts {
            cmd.arg("--font-path").arg(fonts);
        }
        let result = cmd.arg(&typ).arg(&pdf).output().map_err(|e| {
            io::Error::new(e.kind(), format!("cannot run `{}`: {e} (Typst is needed for PDF)", typst.command))
        })?;
        if !result.status.success() {
            let msg = String::from_utf8_lossy(&result.stderr);
            return Err(io::Error::other(format!("Typst could not set {}:\n{msg}", typ.display())));
        }
        written.push(typ);
        written.push(pdf);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_finds_vk_files_below_directories() {
        let dir = std::env::temp_dir().join(format!("verkodo-load-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub/a.vk"), "module a\n").unwrap();
        std::fs::write(dir.join("notes.txt"), "not verkodo").unwrap();
        std::fs::create_dir_all(dir.join(".lake")).unwrap();
        std::fs::write(dir.join(".lake/hidden.vk"), "module hidden\n").unwrap();
        let sources = load(std::slice::from_ref(&dir)).unwrap();
        // An open file wins over the disk.
        let open = HashMap::from([(dir.join("sub/a.vk"), "module b\n".to_string())]);
        let edited = load_with(std::slice::from_ref(&dir), &open).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(sources.files.len(), 1);
        assert!(sources.files[0].path.ends_with("a.vk"));
        assert!(sources.diags.is_empty(), "{:?}", sources.diags);
        assert_eq!(edited.files.len(), 1);
        assert_eq!(edited.files[0].text, "module b\n");
    }
}
