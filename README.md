# verkodo

**Truth code – mathematics and law written formally, checked by machine.** A [falkOSS](https://falkoss.org) project.

verkodo writes knowledge down formally instead of as prose – mathematics first, then German and, later, European law. A concept can have several definitions, joined by bridges that prove them equivalent; a lecture, a guide or a commentary is a path through them; Lean 4 checks the proofs. Text, PDF and web pages in any language are generated from it, and every word in them is explained down to its foundations – the links come from the formalisation, nobody sets them by hand. It is for people who teach, learn, write or apply mathematics and law.

verkodo is at its beginning: a first version of the language reads `.vk` files, checks them, lets Lean check the
proofs and writes HTML pages in which every term explains itself on hover. Version 0.2 is on crates.io.
The idea and the plan are in the [whitepaper](https://github.com/verkodo/whitepaper). The knowledge itself –
mathematics and law, sourced and checked – grows in [verkodo/sciaro](https://github.com/verkodo/sciaro) (Esperanto
for a body of knowledge); [examples/](examples/) here only shows the language.
The documentation is at [verkodo.github.io/verkodo](https://verkodo.github.io/verkodo/) (later verkodo.dev).

| Path | What |
|---|---|
| [crates/](crates/) | the Rust core: `verkodo-syntax` (the language), `-core` (the knowledge graph), `-lean` (the bridge to Lean 4), `-render` (HTML, Typst, Markdown – each a feature), `-sdk` (every command and query – what the command line, the language server, the API and IDEs build on), `-lsp` (the language server), `-api` (HTTP/JSON), `-web` (the web editor, built in), `verkodo` (the command line) |
| [examples/](examples/) | a slice of Analysis I, a simplified § 19 UStG and the income tax scale of § 32a EStG (not legal or tax advice) |
| [lean/](lean/) | the Lean project the generated files are checked in |
| [web/](web/) | the web app – an editor for `.vk` files that checks in the browser (WebAssembly), embedded into the binary |
| [api/](api/) | the API description in OpenAPI, generated from the code |
| [docs/](docs/) | the documentation site – Jekyll, published by GitHub Actions |

The brand kit lives in its own repository, [verkodo/brand](https://github.com/verkodo/brand).

## Try it

From crates.io (with the web editor; Lean and Typst come separately, see below): `cargo install verkodo`. From
this repository:

```sh
cargo run -- check examples              # parse, resolve and check
cargo run -- graph examples              # every declaration and what it refers to
cargo run -- prove examples              # Lean checks definitions, bridges and proofs
cargo run -- render examples -o out --lang de --prove   # HTML with nested hover and Lean's verdict; or --lang en
cargo run -- pdf examples --path guide -o out --prove  # a path as PDF through Typst; every term links to its explanation
cargo run -- lsp                         # the language server for editors (docs/editors.md)
(cd web && pnpm install && pnpm wasm && pnpm build)   # the web editor the binary embeds (crates/verkodo-web/web)
cargo run -- serve                       # the API and the web app on http://127.0.0.1:8080
```

`prove` and `render --prove` need Lean ([elan](https://github.com/leanprover/elan)) and Mathlib: run
`lake exe cache get` once in `lean/`. `pdf` needs [Typst](https://typst.app) (`--typst` names the command; `--fonts`
a directory with the IBM Plex fonts of the brand, otherwise the fonts Typst brings along). Open the pages through a local web server, e.g. `python3 -m http.server -d out`.
The language is described in [docs/language.md](docs/language.md).

## The name

```
vero   truth
kodo   code
       → verkodo
```

*verkodo* joins two Esperanto words: *vero*, truth, and *kodo*, code – a system of signs, and the text of a program. Put together they make a truth code: knowledge written down so that a machine can check that it holds. Both words are in the Reta Vortaro, one of the standard dictionaries; the compound is new. Said ver-KO-do, stressed on the second-to-last syllable like every Esperanto word.

## Principles

- **Everything included** – one edition, every feature free; no paid tiers, licence keys or unlocks.
- **No telemetry** – only connections that are technically needed or that users start themselves.
- **In the open** – issues, plans, reviews and decisions happen in public.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Every commit is signed off under the
[Developer Certificate of Origin](https://developercertificate.org) (`git commit -s`); there is no CLA.

## Licence

[AGPL-3.0-or-later](LICENSES/AGPL-3.0-or-later.txt), an SPDX header in every source file ([REUSE](https://reuse.software)). The
examples are under [CC BY 4.0](LICENSES/CC-BY-4.0.txt), the copies of the brand's artwork under
[CC BY-ND 4.0](LICENSES/CC-BY-ND-4.0.txt); the brand (name, logo, marks)
itself is not covered by any licence.

© 2026 falkOSS – Falcon Lake Open Source Software gUG (haftungsbeschränkt) i. G. and the verkodo contributors
