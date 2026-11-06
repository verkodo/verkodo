---
title: Home
nav_order: 1
---

# verkodo

**Truth code – mathematics and law written formally, checked by machine.** A [falkOSS](https://falkoss.org) project – everything included, no telemetry.

verkodo writes knowledge down formally instead of as prose – mathematics first, then German and, later, European law. A concept can have several definitions, joined by bridges that prove them equivalent; a lecture, a guide or a commentary is a path through them; Lean 4 checks the proofs. Text, PDF and web pages in any language are generated from it, and every word in them is explained down to its foundations – the links come from the formalisation, nobody sets them by hand. It is for people who teach, learn, write or apply mathematics and law.

verkodo is at its beginning; version 0.2 can be tried. [The web editor]({{ '/editor/' | relative_url }}) runs in the
browser, without installing anything – everything but Lean, which checks on your own device; the files stay in the
browser. On your own device, with Rust 1.99 or later:

```sh
cargo install verkodo
verkodo serve      # the web editor at http://127.0.0.1:8080
```

The knowledge written in verkodo – mathematics and law, sourced and checked by Lean – grows in
[sciaro](https://github.com/verkodo/sciaro), starting with § 19 UStG.

What to write: [the language]({{ '/language.html' | relative_url }}); working with it: [editors]({{ '/editors.html' | relative_url }}) and [the API]({{ '/api.html' | relative_url }}).
