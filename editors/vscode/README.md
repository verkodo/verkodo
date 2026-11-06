# verkodo for VS Code

**verkodo – mathematics and law written formally, checked by machine.** This extension makes VS Code an editor for
`.vk` files: highlighting, and the verkodo language server – diagnostics while typing, hover cards that explain every
term, go to definition, the outline, formatting, and Lean's verdict when a file is saved (with a Lean project).

It needs the `verkodo` program:

```sh
cargo install verkodo     # Rust 1.99 or later
```

Settings: `verkodo.path` (the program), `verkodo.lang` (`de` or `en`, the language of hover texts), `verkodo.lean`
(the Lean project, or `false`), `verkodo.style` and `verkodo.width` (how formatting writes a file). The command
**verkodo: Restart the language server** starts it again. More in the
[documentation](https://verkodo.github.io/verkodo/editors.html).

Free software under the AGPL-3.0-or-later, a [falkOSS](https://falkoss.org) project. The icon is the verkodo mark
from [verkodo/brand](https://github.com/verkodo/brand) (CC BY-ND 4.0).
