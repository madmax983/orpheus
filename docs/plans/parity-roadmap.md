# TidalCycles + Faust Parity Roadmap

Status checklist for feature parity with TidalCycles (pattern language) and
Faust (signal graph algebra). Statuses: `[x]` done, `[~]` partial, `[ ]` todo.
File paths point the next contributor at the relevant implementation sites.

## Tidal: pattern transforms

- [x] `fast` / `slow` — `crates/orpheus-lang/src/value.rs` (`PatternRuntime::Fast`/`Slow`)
  - [~] integer factors only; Tidal allows rational and patterned factors
- [x] `rev` — `PatternRuntime::Rev`
- [x] `every` — cycle-localized transforms (`query_transform_cycles`, value.rs)
- [x] `when` — ≈ Tidal `whenmod` (period + offset)
- [x] `within` — windowed transform
- [x] `jux` — `crates/orpheus-lang/src/builtins.rs` (`apply_jux`)
- [~] `sometimes` — cycle-granularity only; Tidal applies per-event
  (per-event behavior is available via `sometimes_by(0.5, ...)`)
- [~] `euclid` — no rotation argument
- [x] `stack` / `overlay` — `PatternRuntime::Stack`
- [x] implicit fastcat — whitespace sequences (`eval_sequence`, eval.rs)
- [x] `palindrome` — `apply_palindrome` (builtins.rs)
- [x] `rand` — `PatternRuntime::Rand`
- [~] `chaos` — ≈ Tidal `shuffle`, but no subdivision-count argument
- [x] patternable controls — `gain`/`pan`/`cutoff`/`lpf`/`hpf`/etc. accept
  number patterns (`apply_sample_numeric_control`, builtins.rs)
- [x] `cat` / `slowcat` / `append` — PR #1382; `PatternRuntime::SlowCat`
  (value.rs), `apply_cat` (builtins.rs); children keep localized cycle counters
- [x] `iter` / `iter_back` — PR #1382; `PatternRuntime::Iter` (value.rs),
  `apply_iter` (builtins.rs)
- [ ] `off` — time-shifted overlay of a transformed copy
- [x] `degrade` / `degrade_by` — per-event random removal, deterministic by
  onset hash; `PatternRuntime::Degrade` (value.rs), `apply_degrade`/
  `apply_degrade_by` (builtins.rs)
- [x] `sometimes_by` / `often` / `rarely` / `almost_always` / `almost_never` —
  per-event probabilistic transforms with exact-complement selection
  (`apply_sometimes_by_probability`, builtins.rs)
- [ ] `euclid` rotation / `euclidInv` / `euclidFull`
- [ ] `chunk` / `rot` — the remaining rotation family beside `iter`
- [ ] `segment` / `range` — sampling continuous patterns into discrete steps
- [ ] `run` / `scan` — integer ramp patterns
- [ ] `irand` — integer random source
- [ ] `choose` / `wchoose` — spec exists:
  `docs/design/specs/probabilistic_pattern_sequencing_spec.md`
- [ ] `shuffle` / `scramble` with an explicit subdivision count

## Tidal: grammar (Orpheus uses real grammar instead of mini-notation strings)

- [x] whitespace sequence (implicit fastcat) — `crates/orpheus-lang/src/grammar/orpheus.pest`
- [x] `( ... )` subdivision group
- [x] `~` rest
- [x] `<a b c>` alternation — PR #1382; `alternation` rule (orpheus.pest),
  `Expr::Alternation` (ast.rs), `eval_alternation`/`eval_alternating_items` (eval.rs)
- [ ] `*` repetition (`bd*2`)
- [ ] `/` slow (`bd/2`)
- [ ] `!` replicate
- [ ] `{ ... }` polymeter
- [ ] `?` random removal
- [ ] in-sequence commas (inline stacks)
- [ ] inline euclid syntax (`bd(3,8)`)

## Faust: graph combinators and primitives

Module: `crates/orpheus-dsp/src/graph/` (ADR 0004).

- [x] all 5 combinators — `seq`/`par`/`split`/`merge`/`rec`
  (`crates/orpheus-dsp/src/graph/combinators.rs`)
- [x] oscillators — `sine`/`saw`/`pulse`/`tri`/`noise`
- [x] `one_pole` filter
- [x] `ladder_filter`
- [x] fixed `delay_line`
- [x] `gain_node`
- [x] `sum` / `passthrough` / `constant` / `wire`
- [x] `soft_sat`
- [x] `Processor` + `pipe`/`bind`
- [ ] biquad filter node
- [ ] SVF (state-variable filter) node
- [ ] ADSR / AR envelope nodes
- [ ] fractional / modulatable delay line
- [ ] sample-playback node
- [ ] `MixNode` adapter — crossfade exists in
  `crates/orpheus-dsp/src/synth/mix.rs` but has no graph adapter
- [ ] `PanNode` — 1-in/2-out equal-power panner
- [ ] engine integration of `graph/` — ADR 0004 follow-up; the graph module is
  currently unreferenced by `engine.rs`/`voice.rs`/`pedal/`
