# 2. Patterns

The heart of Orpheus is its pattern notation. Unlike TidalCycles, this notation
is not a quoted string inside your code — it *is* the code. Orpheus has a real
grammar, so `bd sn cp` is bare source you write directly.

Every form below is paste-able at the `>` prompt. Bind it to a name, `:play`,
and listen. Each one says what you hear.

## Sequences: whitespace is time

Space-separated tokens divide the cycle evenly:

```text
d = bd sn cp sn
```

Four events, one per quarter cycle. Two tokens means two half-cycle events; six
means six equal steps. The number of tokens sets the subdivision.

## Rests: `~`

A tilde is a silent step. It still takes up its share of the cycle:

```text
d = bd ~ sn ~
```

Kick on beat one, snare on beat three, silence between — a backbeat.

## Groups: `( )`

Parentheses squeeze a whole sub-sequence into a single step. Here the cycle has
three steps, and the last step packs two fast hats into the space of one:

```text
d = bd sn (hh hh)
```

The two hats inside the group share the final third of the cycle.

## Alternation: `< >`

Angle brackets play **one** of their entries per cycle, advancing each cycle.
This is how you get variation across loops:

```text
d = bd <sn cp>
```

Cycle 1 plays `bd sn`; cycle 2 plays `bd cp`; cycle 3 back to `bd sn`, and so
on. The alternation counts as a single step.

## Speed up and slow down: `*` and `/`

A tight `*n` on a token repeats it `n` times inside its step; `/n` stretches it
across `n` cycles:

```text
d = bd*2 sn
```

`bd*2` fires two kicks in the first half; `sn` takes the second half. Factors
can be decimals (`hh*1.5`), and `/` is the inverse:

```text
d = bd/2 sn sn sn
```

`bd/2` sounds once every two cycles. (Note the difference from a spaced `bd * 2`,
which is arithmetic, not a step modifier — keep the `*` tight against the token.)

## Replicate: `!`

`!n` copies a step `n` times as separate steps — like typing it out `n` times:

```text
d = bd!3 sn
```

Equivalent to `bd bd bd sn`: four steps total.

## Degrade: `?`

A `?` randomly drops the step some of the time; the removal is deterministic
(seeded by position), so a pattern sounds the same each loop but sparse and
humanised. Add a probability to tune it:

```text
d = hh*8?
```

Eight hats, each with a chance of being silenced. `hh*8?0.3` drops roughly 30%.

## Stacks: `,`

A top-level comma layers whole lines so they play at once:

```text
d = bd ~ bd ~, hh hh hh hh
```

Kick pattern and hat pattern, simultaneously, in one binding. You can also stack
inside a group so the layers share that step's span:

```text
d = (bd sn, hh hh hh)
```

There is also a `stack(...)` function form, useful when the layers are
themselves named patterns — see [chapter 3](03-transforms.md#layering).

## Polymeter: `{ }%n`

Curly braces hold comma-separated subsequences that all step at a shared rate.
By default that rate is the length of the first subsequence; a trailing `%n`
overrides it. The classic polymeter is two lines of different lengths cycling
against each other:

```text
d = {bd sn cp, hh hh}%4
```

Both subsequences emit four steps per cycle, so the three-element line and the
two-element line drift against each other over successive cycles.

## Euclidean rhythms: `(n, k)`

Suffix a token with `(pulses, steps)` to distribute `pulses` hits as evenly as
possible across `steps` slots — the euclidean rhythms behind countless grooves:

```text
d = bd(3,8)
```

Three kicks spread over eight slots (the tresillo). A third argument rotates the
pattern: `bd(3,8,2)`. The arguments can even vary per cycle: `bd(<3 5>,8)`.

## Numbers and decimals

Bare numbers are patterns too — useful as control values and note material:

```text
n = 0 2 4 7
```

Decimals and negatives are allowed (`0.5`, `-3`), which matters when you feed
patterns into controls like `gain` and `pan` in the next chapter.

## Putting notation together

```text
d = bd(3,8), ~ sn ~ sn, hh*8?
```

A euclidean kick, a snare backbeat, and a degraded hat run — three layers, all
from notation, no functions yet. Add functions and this becomes a full groove,
which is exactly where the [next chapter](03-transforms.md) goes.
