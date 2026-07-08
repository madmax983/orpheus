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
- [x] `randcat` / `wrandcat` — one child pattern per cycle chosen uniformly
  (or by interleaved `p1, w1, p2, w2, ...` weights) at deterministic,
  site-salted random; `PatternRuntime::RandCat` (value.rs),
  `apply_randcat`/`apply_wrandcat_patterns` (builtins.rs); children keep
  localized (`cycle div n`) cycle counters like `slowcat`
- [x] `iter` / `iter_back` — PR #1382; `PatternRuntime::Iter` (value.rs),
  `apply_iter` (builtins.rs)
- [x] `off` — time-shifted overlay of a transformed copy; `apply_off`
  (builtins.rs) composes `shift`, the transform, and `stack`
- [x] `degrade` / `degrade_by` — per-event random removal, deterministic by
  onset hash; `PatternRuntime::Degrade` (value.rs), `apply_degrade`/
  `apply_degrade_by` (builtins.rs)
- [x] `sometimes_by` / `often` / `rarely` / `almost_always` / `almost_never` —
  per-event probabilistic transforms with exact-complement selection
  (`apply_sometimes_by_probability`, builtins.rs)
- [ ] `euclid` rotation / `euclidInv` / `euclidFull`
- [x] `chunk` / `rot` — `PatternRuntime::Chunk`/`Rot` (value.rs),
  `apply_chunk`/`apply_rot` (builtins.rs); `chunk_back` sweeps the parts in
  reverse, `rot` rotates event values while onsets stay put
- [x] `segment` / `range` — sampling continuous patterns into discrete steps;
  `PatternRuntime::Segment`/`Range` (value.rs), `apply_segment`/`apply_range`
  (builtins.rs); bare `rand` is auto-invoked in pattern position so
  `rand |> segment(8) |> range(200, 2000) |> cutoff` works end-to-end
- [ ] `run` / `scan` — integer ramp patterns
- [x] `irand` — integer random source; `PatternRuntime::IRand` (value.rs)
- [x] `choose` / `wchoose` — spec:
  `docs/design/specs/probabilistic_pattern_sequencing_spec.md`; constant
  numeric values only in v1 (`wchoose` takes interleaved `v1, w1, v2, w2, ...`
  pairs); `PatternRuntime::Choose` (value.rs), `apply_choose`/`apply_wchoose`
  (builtins.rs)
- [x] `shuffle` / `scramble` with an explicit subdivision count —
  `PatternRuntime::ShuffleSlots` (value.rs), `apply_shuffle_slots`
  (builtins.rs); site-salted per-cycle slot permutation (`shuffle`) or
  independent draws with repeats (`scramble`)

## Tidal: grammar (Orpheus uses real grammar instead of mini-notation strings)

- [x] whitespace sequence (implicit fastcat) — `crates/orpheus-lang/src/grammar/orpheus.pest`
- [x] `( ... )` subdivision group
- [x] `~` rest
- [x] `<a b c>` alternation — PR #1382; `alternation` rule (orpheus.pest),
  `Expr::Alternation` (ast.rs), `eval_alternation`/`eval_alternating_items` (eval.rs)
- [x] `*` repetition (`bd*2`) — `step_modifier` rule (orpheus.pest),
  `Expr::Modified`/`StepOp::Fast` (ast.rs), `eval_modified` (eval.rs)
- [x] `/` slow (`bd/2`) — `StepOp::Slow`; slot-level `slow` with onset-less
  tail fragments on the continuation cycles
- [x] `!` replicate — `StepOp::Replicate`, expanded into separate steps by the
  parser (`push_expanded_step`); bare `!` is not supported
- [x] `{ ... }` polymeter — `polymeter` rule with `%n` override,
  `Expr::Polymeter` (ast.rs), `eval_polymeter` (eval.rs) via `fast(n, slowcat)`
- [x] `?` random removal — `StepOp::Degrade` with optional `?p` probability
  suffix, reusing `PatternRuntime::Degrade` with per-site salts
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
- [x] ADSR / AR envelope nodes — `adsr`/`ar`
  (`crates/orpheus-dsp/src/graph/primitives.rs`)
- [ ] fractional / modulatable delay line
- [ ] sample-playback node
- [x] `MixNode` adapter — `mix_node` wraps the crossfade in
  `crates/orpheus-dsp/src/synth/mix.rs` (`graph/adapters.rs`)
- [x] `PanNode` — `pan`, 2-in (audio, position)/2-out equal-power panner
  (`crates/orpheus-dsp/src/graph/primitives.rs`)
- [ ] engine integration of `graph/` — ADR 0004 follow-up; the graph module is
  currently unreferenced by `engine.rs`/`voice.rs`/`pedal/`
