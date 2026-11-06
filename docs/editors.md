---
title: Editors
nav_order: 4
---

# Editors

`verkodo lsp` is a language server (the Language Server Protocol over stdin and stdout). Any editor that speaks it
gets, for `.vk` files:

- **diagnostics** while you type – syntax, names that cannot be found, types of the legal profile, the order of paths;
- **hover** with what a hover card on the web shows: kind, label, id, parameters, definitions, statements, rules,
  sources – in the language you choose;
- **go to definition** for every declaration, step and name in a formula;
- an **outline** of each file;
- **formatting** in the way of writing you choose (option `style`), or else only tabs in the indentation into
  spaces.

On saving, Lean checks the workspace in the background, if there is a Lean project (`lean/` with a lakefile, or the
one you name). When it is done, its verdict arrives as diagnostics, and Lean and Mathlib names in formulas explain
themselves on hover – type, doc string, module – and lead to their declaration in the Lean and Mathlib sources.
Hovering a tactic in a proof shows its goals before and after it, as Lean prints them – what an infoview shows.
Changing a file drops Lean's verdict until the next save.

Options (`initializationOptions`):

| Option | Value | Default |
|---|---|---|
| `lang` | `"de"` or `"en"` – the language of hover texts | `"de"` |
| `lean` | the Lean project, or `false` for none | `lean/` in the workspace, if it has a `lakefile.toml` |
| `style` | `"delimiters"` (`{ … }`, `"…"`, ```` ``` ````) or `"indentation"` – how formatting writes a file | none: formatting only turns tabs into spaces |
| `width` | the widest line of text, e.g. `100`: formatting breaks longer labels, texts, criteria and sources into lines | none |

The language server reads every `.vk` file below the workspace folders, except hidden folders (such as `.git` and
Lean's `.lake`), `target` and `node_modules`.

## Setting it up

There is no editor extension yet; an editor that lets you name a language server for a file type is enough.

- **Neovim:** tried on 2026-11-06 with Neovim 0.12.6 and the configuration below, unchanged: it attaches to `.vk`
  files with the repository as root; hover, go to definition, the outline and diagnostics while typing work; after
  saving, Lean's verdict arrived after about a minute (all example files), and then `tendsto_nhds_unique` explained
  itself and led into Mathlib's sources.
- **Helix:** the configuration is what Helix documents for a language server of its own. **OPEN:** not tried yet.
- **VS Code** and others: **OPEN** – they need an extension or a generic language client.

**Helix** (`languages.toml`):

```toml
[language-server.verkodo]
command = "verkodo"
args = ["lsp"]
config = { lang = "en" }

[[language]]
name = "verkodo"
scope = "source.verkodo"
file-types = ["vk"]
roots = ["lean"]
comment-token = "//"
language-servers = ["verkodo"]
```

**Neovim** (0.11 or later, `init.lua`):

```lua
vim.filetype.add({ extension = { vk = "verkodo" } })
vim.lsp.config("verkodo", {
  cmd = { "verkodo", "lsp" },
  filetypes = { "verkodo" },
  root_markers = { "lean", ".git" },
  init_options = { lang = "en" },
})
vim.lsp.enable("verkodo")
```

## In the browser

`verkodo serve` (or `pnpm dev` in `web/`) opens a web editor: `.vk` files in CodeMirror, checked in the browser by
verkodo itself, compiled to WebAssembly – the same parser, analysis and hover cards as above, without a server.
Diagnostics appear while typing, hover shows the cards, F12 or Ctrl/⌘-click goes to a definition, a side panel shows
the outline and a live preview of the pages. It starts with the examples; the files stay in that browser
(`localStorage`) and are sent nowhere.

The same editor runs [here in the browser]({{ '/editor/' | relative_url }}), published with this documentation
(`pnpm build:pages`): everything below but Lean, which needs a `verkodo serve --lean` on your own device.

It works like an IDE:

- a toolbar to **check** (the problems of all files, Ctrl/⌘-Shift-M), **compile** with Lean (Ctrl/⌘-Shift-B, see
  below), **debug** (F5) and **format** (Shift-Alt-F) in the way of writing of the settings;
- **light, dark or as the system** – three icons at the top; the preview follows;
- **settings** (Ctrl/⌘-,): the language – of the interface (German or English) and of labels and texts, from
  translations where there are some –, the way of writing for formatting and the widest line of text, formatting when switching files, the size
  of the font, wrapping of long lines; kept in that browser;
- an **overview of the keyboard shortcuts** (F1);
- a panel below with **problems**, **Lean**, **goals** (Ctrl/⌘-Shift-G: the goals of the tactic at the cursor,
  once Lean has checked) and **debug**.

**Debug** evaluates a norm: choose it, give its inputs (`ja`/`nein`, numbers also written the German way, `40.000,99`,
dates), and every value and rule shows its value – exactly, with fractions as in Lean – and, for a rule, its default
and each exception tried, and which one decided. A click on a rule goes to it. Judgements are inputs too, marked as
such.

With `verkodo serve --lean lean`, a button **Check with Lean** sends the files to that server: Lean checks them there
(one check at a time, about half a minute for the examples) and its verdict comes back into the editor – Lean's
diagnostics, the counts, "checked by Lean" on cards and in the preview, and Lean and Mathlib names that explain
themselves on hover. The verdict holds until the next change, as in the language server. Lean runs the code it is
given, so `--lean` belongs only where whoever can reach the server may run code on that device (`serve` listens on
127.0.0.1 only).

The server's Content-Security-Policy allows nothing inline but what carries a nonce new with every page, and
WebAssembly (`'wasm-unsafe-eval'`, not `eval`).

A prototype (2026-11-06). **OPEN:** a time limit for Lean and a sandbox before `--lean` faces others; opening and
saving files on disk; search, completion, renaming; going to declarations in Mathlib from the browser.
