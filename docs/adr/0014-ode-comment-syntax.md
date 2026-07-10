# ADR 0014: Line Comment Syntax for `.ode` Source

- Status: Accepted
- Date: 2026-07-10

## Context

Orpheus song files (`.ode`) had no comment syntax. Every character on a line
was significant, so the only place to document a script was an external file
(see `docs/examples/README.md`, which carried a line-by-line feature map for
`reference_song.ode`). This forced documentation to live apart from the code it
described and drift out of step with it. `#`, `//`, and trailing `#` all failed
to parse (`expected identifier`); the grammar's `padding` rule consumed only
whitespace and newlines.

Two markers were considered:

- `//` line comments — familiar from Rust/JS/Strudel.
- `#` line comments — collision-free by construction (`#` has no meaning
  anywhere in the grammar).

## Decision

**`//` line comments, running to end-of-line.** They may stand on their own
line or trail an expression on the same line. There is no block-comment form.

`//` is unambiguous in this grammar. There is no division operator (arithmetic
is only `+` and `*`), and the tight mini-notation slow modifier `/n` requires a
digit immediately after the slash (`bd/2`), so a `/` followed by another `/`
can never begin an expression token. `#` was the fallback had `//` collided; it
did not, so the more familiar marker wins.

## Consequences

- Grammar (`crates/orpheus-lang/src/grammar/orpheus.pest`): a new silent
  `comment = _{ "//" ~ (!NEWLINE ~ ANY)* }` rule is folded into both `padding`
  and `required_padding`, so comments are skipped anywhere whitespace is.
- Parser (`crates/orpheus-lang/src/parser.rs`): the pre-pass that splits a
  source into per-binding chunks is made comment-aware. A `strip_line_comment`
  helper drops a line's `//` tail (respecting string literals, so
  `sample("a//b")` keeps its slashes), and `contains_binding_content` keeps a
  leading banner comment attached to the following binding instead of splitting
  it off as its own (unparseable) chunk. Bracket-nesting and unmatched-paren
  scans ignore commented text.
- A `//` sequence inside a string literal is data, not a comment — guarded at
  both the grammar level (the `string` atomic rule) and the line-splitting
  pre-pass.
- The example scripts (`reference_song.ode`, `tutorial_capstone.ode`) are now
  self-documenting; `docs/examples/README.md` keeps the high-level overview and
  points readers to the inline commentary.
