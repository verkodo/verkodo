---
title: The language
nav_order: 2
---

# The verkodo language (draft 0)

verkodo writes knowledge down formally instead of as prose. Text, PDF and web pages are generated from it in any
language, and every word in them can be explained by hovering, because each name points to the declaration it comes
from – the links are derived from the formalisation, nobody sets them by hand.

One shared **knowledge layer** carries two **logic profiles**:

| | Mathematics | Law |
|---|---|---|
| Declarations | `concept` (with `variant`s and `bridge`s), `theorem`, `example` | `norm` with `input`, `constant`, `rule` |
| Formulas | Lean 4 (Mathlib), checked by Lean from milestone M2 | a small expression language with types |
| Shared | `label`, `source`, `remark`, `path`, stable ids, references, dependencies | |

## Files and ids

A file starts with `module a.b`. Every declaration gets the stable id `<module>.<name>`, variants and members
`<module>.<name>.<part>`. Names resolve in the module first, then as full ids. Files end in `.vk`.

## Two ways of writing

verkodo can be written with delimiters, where line breaks do not matter: `{ … }` around blocks, `"…"` around text
and one line of Lean, ```` ``` ```` around longer Lean code or text. Or by indentation, without any of them: a block
is the lines indented deeper than the line of its keyword, and text is the rest of the line – or, if the line ends,
the lines below indented deeper. Both mean the same and may be mixed; most examples use delimiters,
`examples/estg/p32a.vk` indentation.

````text
theorem limit_unique {
  label de "Eindeutigkeit des Grenzwerts"
  statement lean "converges a L → converges a M → L = M"
  proof lean ```
    fun h₁ h₂ => tendsto_nhds_unique h₁ h₂
  ```
}
````

```text
theorem limit_unique
  label de Eindeutigkeit des Grenzwerts
  statement lean converges a L → converges a M → L = M
  proof lean
    fun h₁ h₂ => tendsto_nhds_unique h₁ h₂
```

Text without quotes follows `lean`, `source` at the start of a line, and `label`, `text` or `criteria` with a
language at the start of a line. It runs to the end of the line – a `//` there is part of it, and Lean's comments are
`--` anyway –, so on one line with a closing brace (`variant p { lean "0 < x" }`) the quotes are needed. Indentation
counts in columns; a tab counts up to the next multiple of two, one level of the usual two spaces.

Texts – labels, a remark's text, criteria, sources – are prose: in a block (between fences or by indentation) a line
break is a space and a blank line separates paragraphs, so a text may be broken into lines anywhere without changing
it. `"…"` is word for word (`\n` is a line break there), and so is Lean code, in every form.

`verkodo fmt --style delimiters` or `--style indentation` writes files one way or the other (`--check` changes
nothing and names what would change); without `--style` it only turns tabs in the indentation into spaces. With
`--width 100`, texts longer than a line of 100 characters are broken into lines – a one-line text becomes a block –,
and shorter blocks become one line again; Lean code and expressions are never broken. It keeps
every entry's words, the layout of expressions, comments and blank lines, and changes only the frame – braces or
indentation, levels of two spaces, how texts are delimited. It reads its result again and refuses if the meaning
would change; a file with errors is left as it is. The language server formats the same way (options `style` and
`width`).

## Shared declarations

```text
label de "konvergiert gegen"      // a label per language; English is the fallback
source "§ 19 Abs. 1 UStG"         // where it comes from

remark limit_notation {
  about limit_unique              // what it is about (checked)
  text de "…"                     // informal text, per language
}

path analysis1 {                  // a lecture, a guide, a commentary
  label de "Analysis I – Folgen"
  step converges.eps_n            // choose a variant of a concept
  step limit_unique
}
```

A **path** is a route through the knowledge. verkodo checks its order: a step that uses something the path has not
introduced yet gets a warning. A second variant of a concept is fine once a bridge connects it to the first.

## Translations

A label in German law is the law's own word; its English is already a translation. So a module names the languages it
is written in – `module ustg.p19 original de, en` – and texts in other languages go into translation files, one per
module and language by convention (`folgen.fr.vk` next to `folgen.vk`):

```text
translation analysis.folgen

converges
  label fr converge vers
limit_notation
  label fr Notation
  text fr Comme la limite est unique, on peut parler de la limite et écrire lim a.
```

A translation names a declaration relative to the module and gives its `label`, a remark's `text` or a judgement's
`criteria` – the same lines as at the declaration, in either way of writing. verkodo checks that the declaration
exists and has that kind of text, that no text is translated twice, and warns about texts at a declaration in a
language that is not an original one, and about translations into an original language. Pages in a language take
its texts from the declaration, else from a translation, else English. `verkodo translations <files> --missing` says
for each module and language how many texts are translated and which are missing.

## Mathematics

```text
concept converges {
  params lean "(a : ℕ → ℝ) (L : ℝ)"
  variant eps_n  { lean "∀ ε > 0, ∃ N : ℕ, ∀ n ≥ N, |a n - L| < ε" }
  variant filter { lean "Filter.Tendsto a Filter.atTop (nhds L)" }
  bridge eps_n <-> filter { proof lean "…" }
}

theorem limit_unique {
  params lean "(a : ℕ → ℝ) (L M : ℝ)"
  using converges.filter          // written against this variant; otherwise the first variant
  statement lean "converges a L → converges a M → L = M"
  proof lean "fun h₁ h₂ => tendsto_nhds_unique h₁ h₂"   // missing: reported as OPEN
}
```

Lean code is written either as a string (`lean "…"`, with `\"` and `\n` as escapes) or, for longer code, as a
block between fences – no escapes, as many lines as needed:

````text
theorem const_converges {
  params lean "(c : ℝ)"
  statement lean "converges (fun _ => c) c"
  proof lean ```
    by
      intro ε hε
      exact ⟨0, fun n _ => by simpa using hε⟩
  ```
}
````

The code starts on the line after the opening fence and ends before the closing one; the indentation all its lines
share is removed, so the block may be indented like the verkodo around it while Lean sees its own indentation.

`verkodo prove` turns each module into one Lean file: every variant becomes an `abbrev` (a `Prop`), every bridge a
theorem `concept.l_iff_r` about the two definitions plus `concept.l_eq_r` (the definitions are equal), every theorem
is stated with the variants it is written against. For each path, theorems whose concepts the path defines with a
different variant are **carried over** along the bridges (`rw [concept.l_eq_r]`) – so a proof written for ε-N holds
in the filter lecture, and the other way round. Bridges chain: with `eps_n <-> filter` and `filter <-> metric`, a
theorem written for `eps_n` also holds in a path that chose `metric`, through the shortest chain of bridges. A theorem
carried over is only as good as the theorem itself and every bridge on the way – one bridge without a proof leaves it
OPEN.

A concept without a type is a statement about its parameters. With `type` it defines an **object** – a number, a
function, a set – and its bridges are equalities, written with `=`:

```text
concept distance {
  params lean "(x y : ℝ)"
  type lean "ℝ"
  variant absolute { lean "|x - y|" }
  variant root     { lean "Real.sqrt ((x - y) ^ 2)" }
  bridge absolute = root { proof lean "(Real.sqrt_sq_eq_abs (x - y)).symm" }
}
```

Each variant becomes a definition of that type (`noncomputable abbrev distance.absolute (x y : ℝ) : ℝ`; Lean need
not compute the reals to reason about them), each bridge a theorem `distance.absolute_eq_root_at` about the two
definitions plus `distance.absolute_eq_root` (the definitions are equal). Theorems about the object are carried over
exactly like theorems about statements. A bridge written with the wrong sign – `<->` between objects, `=` between
statements – is an error.

Inside its own definitions Lean opens the namespace of the concept, so a variant's name hides a Lean name of the same
spelling there: a variant called `dist` would make `dist (a n) L` refer to the variant itself. Give variants names
that are not Lean names used in the concept's formulas. Lean's verdict (checked, open, rejected) comes back as diagnostics on
the `.vk` source and into the HTML.

Inside Lean formulas, names of verkodo declarations and parameters are found by a light reading of the snippet. Every
other name comes from Lean itself: `verkodo prove` elaborates the generated files with a small Lean program that also
reports each identifier Lean resolved to a constant – its full name, type, doc string and module – and verkodo carries
these back onto the `.vk` source. In HTML from `verkodo render --prove`, `nhds` or `tendsto_nhds_unique` then
explain themselves on hover like the terms of verkodo; without `--prove` only dotted names (`Filter.Tendsto`) are
marked, without an explanation. Names that notation stands for (such as `⟨…⟩`) are not reported.

## Law

```text
norm small_business {
  valid from 2025-01-01
  input turnover_prev : money
  constant limit_prev : money = 25_000 EUR
  rule within_limits : bool = turnover_prev <= limit_prev
  rule exempt : bool {
    default resident and within_limits
    unless waived then false      // an exception overrides the default
  }
}
```

Types: `bool`, `money`, `int`, `decimal`, `date` – and the enumerations, units and records a module declares
(below). Operators: `and`, `or`, `not`, `< <= > >= == !=`, `+ - * /`, `if … then … else …`. Amounts carry a
currency (`5 EUR`); money times a number is money. `19 %` is the number 0.19.

Functions: `min(…)`, `max(…)`, `abs(x)`; on whole numbers `div(a, b)` and `mod(a, b)` (rounding down, the remainder
never negative); the rounding functions below; and those on dates further down.

Rounding is part of the law, so it is part of the language: `round_down(x, 1 EUR)` ("abrunden"),
`round_up(x, 1 EUR)` ("aufrunden") and `round(x, 0.01 EUR)` (to the nearest step, halves away from zero –
"kaufmännisch"). The step is written out and positive, an amount for an amount (`0.5 EUR`, not `0.5`). § 32a EStG
rounds twice – the taxable income down to full euros, then the tax:

```text
rule x : money = round_down(taxable_income, 1 EUR)
rule tax : money = round_down(tax_exact, 1 EUR)
```

In Lean these are `verkodo.round_down`, `verkodo.round_up` and `verkodo.round` over ℚ (floor and ceiling of
`x / step`), exact like the law: at 40,000 euros the scale gives 7,209.63… euros, rounded down 7,209 – with
`round` it would be 7,210, and Lean rejects the example.
Exceptions are tried in order: the first `unless` whose condition holds gives the value, otherwise the default.

### Tables

Where the law gives a value by ranges or by cases, a rule is written as a table: `by` names what it goes by, each
row gives a value, `otherwise` the rest. By ranges, every row reaches `up to` its bound – the bounds go up, so the
rows neither leave a gap nor overlap; verkodo checks that the bounds written out ascend and that `otherwise` covers
what lies above the last:

```text
rule tax_exact : money {
  by x
  up to basic_allowance: 0 EUR
  up to 17_799 EUR: (914.51 EUR * y + 1_400 EUR) * y
  …
  otherwise: 0.45 * x - 19_470.38 EUR
}
```

By an enumeration, the rows name its values; every value needs a row, or `otherwise` takes the rest:

```text
rule rate : decimal { by established domestic: 19 % eu_other: 7 % otherwise: 0 % }
```

A table is the default of its rule – `unless` may still override it. In Lean it is a chain of `bif` or a `match`.

### Enumerations, units and records

A module declares the types its norms share:

```text
enum residence {
  label de "Ansässigkeit"
  value domestic { label de "Inland" }
  value eu_other { label de "übriges Gemeinschaftsgebiet" }
  value third_country
}

unit km { label de "Kilometer" }

record supply {
  field amount : money
  field kind : residence
}
```

An **enumeration** has a fixed set of values, each with its labels; an input of it holds exactly one of them, so
"both" and "neither" are not cases (`input established : residence`). Values are written with their enumeration
(`established == residence.domestic`) and compared with `==` and `!=`. A **unit** is an amount other than money:
`30 km`; only amounts of the same unit add up or compare, an amount times a number stays an amount, and an amount
divided by its unit is a number (`distance / 1 km`). A unit is found by its name in any module, as long as only one
declares it. A **record** groups fields: an input may be one record (`owner.age`) or a list of records
(`input supplies : list supply`). Types of other modules are named by their path (`de.ustg.p1.supply`).

Over a list, `sum`, `count`, `all` and `any`, each with an optional `where`:

```text
rule domestic : money = sum(s.amount for s in supplies where s.kind == residence.domestic)
rule many : int = count(s in supplies)
rule none_abroad : bool = all(s.kind == residence.domestic for s in supplies)
```

In Lean an enumeration is an `inductive` type, a record a `structure`, a list a `List`; the aggregates are
`List.filter`, `map` and `sum`, `length`, `all` and `any`.

### Dates

Dates are compared, and days added: `day + 14` is two weeks later, `deadline - day` the days between. Functions take
a date apart and put one together:

| Function | Gives |
|---|---|
| `year(d)`, `quarter(d)`, `month(d)`, `day(d)` | the year, the quarter (1–4), the month (1–12), the day of the month |
| `weekday(d)` | the day of the week, 1 Monday … 7 Sunday |
| `date(y, m, d)` | the date – counting on past the end: month 13 is January of the next year, day 0 the last day of the month before |
| `last_day(y, m)` | the last day of the month |
| `add_months(d, n)`, `add_years(d, n)` | `n` months or years later, on the last day of the month where that month is shorter (§ 188 (2), (3) BGB) |

So "the last day of February of the second calendar year after the taxable period" is
`last_day(year(period_end) + 2, 2)`, and "from the beginning of the calendar quarter after it is received"
`date(year(received), 3 * quarter(received) + 1, 1)`. What depends on a place – public holidays, a working day under
§ 193 BGB – is not built in: each body of law declares it in a module of its own.

In Lean these are `verkodo.year`, `verkodo.date`, `verkodo.add_months` and so on over day numbers – the same
formulas as in verkodo, so a deadline is proved `by decide`.

### Norms that use norms

A rule may use a rule of another norm – of this module or another – for inputs given by name; every input of that
norm is given once:

```text
rule turnover_prev : money = total_turnover.total(taxable = taxable_prev, exempt_listed = exempt_prev, …)
```

A constant of another norm needs no inputs: `small_business.limit_prev`. In Lean the call is the other norm's
function applied to its inputs (`total_turnover.total { taxable := …, … }`); the generated file carries the
enumerations, records and norms of the modules it uses.

`verkodo prove` turns a norm into Lean: a namespace with the inputs as a structure `Inputs`, every constant a
definition and every rule a function of the inputs (`def exempt (i : Inputs) : Bool`). Truth values are `Bool`,
whole numbers and dates `ℤ`, other numbers and amounts rationals (`ℚ`) – exact, as the law computes; where a whole
number meets a fraction it is converted (`↑n`). Lean type-checks the rules once more, and properties of a norm are ordinary theorems and examples about these
functions:

```text
theorem waiver_excludes {
  params lean "(i : small_business.Inputs)"
  statement lean "i.waived = true → small_business.exempt i = false"
  proof lean "by intro h; simp [small_business.exempt, h]"
}

example at_the_limits {   // a case with its expected result
  statement lean "small_business.exempt { resident := true, turnover_prev := 25000, turnover_curr := 100000, waived := false } = true"
  proof lean "by norm_num [small_business.exempt, small_business.within_limits, small_business.limit_prev, small_business.limit_curr]"
}
```

If a rule changes so that a property no longer holds – say `<` instead of `<=` for a limit that may be reached –
Lean rejects the proof and the error lands on the theorem in the `.vk` file. Not exported yet (a warning instead):
constants that depend on inputs.

### Obligations, permissions, prohibitions

A rule may say what someone must, may or must not do. It is written `obligation`, `permission` or `prohibition`
instead of `rule`, has no type – it is a truth value: whether the obligation, permission or prohibition applies – and
`when` instead of `default`:

```text
prohibition revoke_early {
  label de "darf den Verzicht noch nicht widerrufen"
  when waived and waiver_years < 5
}

permission revoke {
  label de "darf den Verzicht widerrufen"
  when waived and waiver_years >= 5
}
```

Exceptions work as in every rule. In Lean these are functions to `Bool` like other rules, so how they fit together is
an ordinary theorem: that an act is never both permitted and forbidden, or that after a waiver one of the two always
applies. Lengthen the prohibition to `< 6` and Lean rejects that theorem. verkodo does not build a deontic logic of
its own (what follows from an obligation, conflicts between norms); such properties are stated and proved.

### Judgements

Some words of the law are not facts that can be looked up but **undetermined legal terms** – "appropriate", "for an
important reason", "expected". verkodo does not pretend to compute them: a `judgement` is an input that someone has
to assess and justify, with the criteria by which it is judged:

```text
judgement turnover_curr : money {
  label de "voraussichtlicher Umsatz im laufenden Kalenderjahr"
  criteria de "Der sachgerecht geschätzte Gesamtumsatz des laufenden Jahres …"
  source "§ 19 Abs. 1 Satz 1 UStG a. F."
}
```

A judgement without criteria gets a warning. Every rule and value shows which judgements its result depends on,
directly or through other rules – what a person has to assess before the answer is known. In Lean a judgement is an
input like any other (a field of `Inputs`, marked in its doc string): properties hold for every way it may be judged.

### Versions

Law changes; each version of a provision is a norm of its own, and the later one says which one it `replaces`:

```text
norm small_business_2020 {
  valid from 2020-01-01 to 2024-12-31
  constant limit_prev : money = 22_000 EUR
  …
}

norm small_business {
  valid from 2025-01-01
  replaces small_business_2020
  constant limit_prev : money = 25_000 EUR
  …
}
```

verkodo checks that the versions follow each other: the earlier one must end the day before the later one begins
(an overlap or a version without end is an error, a gap a warning), a version has one successor, and dates must exist
(`2025-02-30` is an error). Which version a case falls under is the one whose `valid from … to …` contains its date.

In Lean, dates are day numbers (`ℤ`, days since 1970-01-01): a `date` input is an integer, a date in a rule its
number. Each norm gets `valid_from`, `valid_to` and `in_force (d : ℤ) : Bool`, and every generated file with norms
defines `verkodo.day y m d`, so that statements can name days: `small_business.in_force (verkodo.day 2025 1 1) = true`
is proved `by decide`. Properties across versions are ordinary theorems about both – for instance that nobody exempt
under the old limits loses the exemption under the new ones with the same figures.

## Open design questions

- Objects whose parameters mention a concept (the limit of a convergent sequence needs a proof that it converges);
  structures with several fields in mathematics.
- The legal profile: a shorter way to write cases in Lean (all inputs of a norm are named); lists of other values
  than records; several currencies; modules that use each other's norms in a circle.
- Time: a case that spans versions (a calendar year across a change of law); transitional provisions; one
  function over all versions of a provision.
- Translations: noticing that a translation is out of date once the original changes.
- Deontic notions: who bears an obligation, which act several rules are about (so that conflicts can be found
  without writing the theorem), periods as a notion of their own (beginning and end by §§ 187, 188 BGB as functions
of dates exist, the rules of a period not yet). For judgements: the justification of a case, and typical cases
  (*Fallgruppen*) as examples.
