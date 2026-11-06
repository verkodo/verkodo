// SPDX-License-Identifier: AGPL-3.0-or-later
//! The command line of verkodo: checking knowledge, letting Lean check it and writing HTML;
//! the API with the web app, and the API's description.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};

use verkodo_sdk::knowledge::{self, Diagnostic, Report, Severity, Status};

/// Truth code – mathematics and law written formally, checked by machine.
#[derive(Debug, Parser)]
#[command(name = "verkodo", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Parses, resolves and checks `.vk` files.
    Check {
        #[command(flatten)]
        input: Input,
    },
    /// Prints every declaration and what it refers to.
    Graph {
        #[command(flatten)]
        input: Input,
        /// The language of the labels.
        #[arg(long, default_value = "de")]
        lang: String,
    },
    /// Exports to Lean 4 and lets Lean check definitions, bridges and proofs.
    Prove {
        #[command(flatten)]
        input: Input,
        #[command(flatten)]
        lean: Lean,
    },
    /// Writes HTML pages with nested hover – an index and one page per path.
    Render {
        #[command(flatten)]
        input: Input,
        /// Where the pages go.
        #[arg(short, long, default_value = "out")]
        out: PathBuf,
        /// The language of the pages.
        #[arg(long, default_value = "de")]
        lang: String,
        /// Lets Lean check first and shows its verdict on the pages.
        #[arg(long)]
        prove: bool,
        #[command(flatten)]
        lean: Lean,
    },
    /// Writes paths as PDF through Typst – every term a link to its step or to the appendix.
    Pdf {
        #[command(flatten)]
        input: Input,
        /// Only this path (its id, or its name within the module); otherwise every path.
        #[arg(long)]
        path: Option<String>,
        /// Where the Typst documents and PDFs go.
        #[arg(short, long, default_value = "out")]
        out: PathBuf,
        /// The language of the documents.
        #[arg(long, default_value = "de")]
        lang: String,
        /// Lets Lean check first and shows its verdict, and what Lean says about Lean and Mathlib names.
        #[arg(long)]
        prove: bool,
        #[command(flatten)]
        lean: Lean,
        /// The Typst command.
        #[arg(long, default_value = "typst")]
        typst: String,
        /// A directory with the IBM Plex fonts; without it the fonts Typst brings along.
        #[arg(long)]
        fonts: Option<PathBuf>,
    },
    /// How far each language of the translation files is: texts translated of those written, and which are missing.
    Translations {
        #[command(flatten)]
        input: Input,
        /// Also lists every missing text.
        #[arg(long)]
        missing: bool,
    },
    /// Writes `.vk` files in one way of writing – with delimiters or by indentation – and tabs as spaces. Files with
    /// errors are left as they are.
    Fmt {
        #[command(flatten)]
        input: Input,
        /// `delimiters` ({ … }, "…", ```) or `indentation`; without it, only tabs become spaces.
        #[arg(long)]
        style: Option<String>,
        /// The widest a line of text may be: longer labels, texts, criteria and sources are broken into lines (Lean
        /// code and expressions never). Needs `--style`.
        #[arg(long)]
        width: Option<usize>,
        /// Changes nothing; names the files that would change and fails if there are any.
        #[arg(long)]
        check: bool,
    },
    /// The language server (LSP over stdin and stdout) for editors: diagnostics, hover, go-to-definition, outline.
    Lsp,
    /// The API and the web app on this device.
    Serve {
        /// The port on 127.0.0.1.
        #[arg(long, default_value_t = 8080)]
        port: u16,
        /// Lets the web editor have its files checked by Lean in this Lean project. Lean runs the code it is given:
        /// switch it on only where whoever can reach the server may run code on this device.
        #[arg(long)]
        lean: Option<PathBuf>,
    },
    /// Prints the API's description in OpenAPI – what api/openapi.json holds.
    Openapi,
}

#[derive(Debug, Args)]
struct Input {
    /// `.vk` files, or directories to search for them.
    #[arg(required = true)]
    paths: Vec<PathBuf>,
}

#[derive(Debug, Args)]
struct Lean {
    /// The Lean project (lakefile with Mathlib); the generated files go to its `generated/`.
    #[arg(long = "lean", default_value = "lean")]
    dir: PathBuf,
}

fn main() -> anyhow::Result<ExitCode> {
    match Cli::parse().command {
        Command::Check { input } => knowledge_command(&input.paths, None, |_, _, _| Ok(())),
        Command::Graph { input, lang } => knowledge_command(&input.paths, None, |sources, analysis, _| {
            print!("{}", analysis.graph(sources, &lang));
            Ok(())
        }),
        Command::Prove { input, lean } => knowledge_command(&input.paths, Some(&lean.dir), |_, _, _| Ok(())),
        Command::Render { input, out, lang, prove, lean } => {
            let lean_dir = prove.then_some(lean.dir.as_path());
            knowledge_command(&input.paths, lean_dir, |sources, analysis, report| {
                for page in knowledge::render(sources, analysis, report, &out, &lang)? {
                    eprintln!("wrote {}", page.display());
                }
                Ok(())
            })
        }
        Command::Pdf { input, path, out, lang, prove, lean, typst, fonts } => {
            let lean_dir = prove.then_some(lean.dir.as_path());
            let typst = knowledge::TypstOptions { command: typst, fonts };
            knowledge_command(&input.paths, lean_dir, |sources, analysis, report| {
                let paths = knowledge::select_paths(analysis, path.as_deref());
                if paths.is_empty() {
                    anyhow::bail!("no path {}", path.as_deref().map(|p| format!("`{p}`")).unwrap_or_default());
                }
                for file in knowledge::pdf(sources, analysis, report, &paths, &out, &lang, &typst)? {
                    eprintln!("wrote {}", file.display());
                }
                Ok(())
            })
        }
        Command::Translations { input, missing } => knowledge_command(&input.paths, None, |sources, analysis, _| {
            let coverage = analysis.coverage(sources);
            if coverage.is_empty() {
                println!("no translations");
            }
            for ((module, lang), c) in coverage {
                let percent = (c.translated * 100).checked_div(c.total).unwrap_or(100);
                println!("{module} {lang}: {} of {} texts ({percent} %)", c.translated, c.total);
                if missing {
                    for (id, field) in c.missing {
                        println!("  {id} {}", field.keyword());
                    }
                }
            }
            Ok(())
        }),
        Command::Fmt { input, style, width, check } => fmt(&input.paths, style.as_deref(), width, check),
        Command::Lsp => {
            verkodo_lsp::stdio().map_err(|e| anyhow::anyhow!(e))?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Serve { port, lean } => {
            let ready = |address: std::net::SocketAddr| println!("verkodo at http://{address} – Ctrl-C ends");
            tokio::runtime::Runtime::new()?.block_on(verkodo_api::serve(
                verkodo_web::around(verkodo_api::router(lean)),
                port,
                ready,
            ))?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Openapi => {
            println!("{}", verkodo_api::openapi());
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// `verkodo fmt`: every `.vk` file of the paths in the way of writing asked for.
fn fmt(paths: &[PathBuf], style: Option<&str>, width: Option<usize>, check: bool) -> anyhow::Result<ExitCode> {
    let style = match style {
        None => None,
        Some("delimiters") => Some(knowledge::Style::Delimiters),
        Some("indentation") => Some(knowledge::Style::Indentation),
        Some(other) => anyhow::bail!("no way of writing `{other}` – `delimiters` or `indentation`"),
    };
    let sources = knowledge::load(paths)?;
    let mut changed = 0;
    let mut failed = 0;
    for file in &sources.files {
        let result = match style {
            Some(style) => knowledge::format_with(&file.text, knowledge::FormatOptions { style, width }),
            None => Ok(knowledge::spaces_for_tabs(&file.text)),
        };
        match result {
            Ok(text) if text != file.text => {
                changed += 1;
                if check {
                    eprintln!("would change {}", file.path);
                } else {
                    std::fs::write(&file.path, text)?;
                    eprintln!("wrote {}", file.path);
                }
            }
            Ok(_) => {}
            Err(why) => {
                failed += 1;
                eprintln!("{}: {why}", file.path);
            }
        }
    }
    Ok(if failed > 0 || (check && changed > 0) { ExitCode::FAILURE } else { ExitCode::SUCCESS })
}

/// Loads and checks the sources, lets Lean check them if `lean_dir` is given, prints the diagnostics, runs `then` and
/// fails if there are errors.
fn knowledge_command(
    paths: &[PathBuf],
    lean_dir: Option<&Path>,
    then: impl FnOnce(&knowledge::Sources, &knowledge::Analysis, Option<&Report>) -> anyhow::Result<()>,
) -> anyhow::Result<ExitCode> {
    let sources = knowledge::load(paths)?;
    let analysis = knowledge::analyze(&sources);
    let mut diags: Vec<Diagnostic> = sources.diags.iter().chain(&analysis.diags).cloned().collect();
    let has_errors = |d: &[Diagnostic]| d.iter().any(|x| x.severity == Severity::Error);

    let mut report = None;
    if let Some(lean_dir) = lean_dir {
        if has_errors(&diags) {
            eprintln!("not exporting to Lean: fix the errors first");
        } else {
            eprintln!("checking with Lean in {} …", lean_dir.display());
            let outcome = knowledge::prove(&sources, &analysis, lean_dir)?;
            diags.extend(outcome.diags);
            summary(&outcome.report);
            report = Some(outcome.report);
        }
    }

    diags.sort_by_key(|d| (d.file, d.span.start, d.severity));
    for d in &diags {
        eprintln!("{}", sources.format(d));
    }
    let errors = diags.iter().filter(|d| d.severity == Severity::Error).count();
    let warnings = diags.len() - errors;
    then(&sources, &analysis, report.as_ref())?;
    eprintln!(
        "{} files, {} declarations, {errors} errors, {warnings} warnings",
        sources.files.len(),
        analysis.symbols.len()
    );
    Ok(if errors > 0 { ExitCode::FAILURE } else { ExitCode::SUCCESS })
}

fn summary(r: &Report) {
    let count = |f: fn(&Status) -> bool| r.status.values().filter(|s| f(s)).count();
    eprintln!(
        "Lean: {} checked, {} open (sorry), {} rejected",
        count(|s| *s == Status::Checked),
        count(|s| *s == Status::Open),
        count(|s| matches!(s, Status::Failed(_)))
    );
}
