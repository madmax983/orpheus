# TidalCycles + Faust Parity Roadmap

Status checklist for feature parity with TidalCycles (pattern language) and
Faust (signal graph algebra). Statuses: `[x]` done, `[~]` partial, `[ ]` todo.
File paths point the next contributor at the relevant implementation sites.

## Tidal: pattern transforms

- [x] `fast` / `slow` — `crates/orpheus-lang/src/value.rs` (`PatternRuntime::Fast`/`Slow`)
  - [x] rational factors — `fast(1.5, ...)` / `slow(0.5, ...)`; decimal
    literals convert to exact rationals (numerator and denominator each
    bounded by 1024; `extract_positive_rational_factor`, builtins.rs)
  - [x] patterned factors (Tidal `fast "<1 2>"`) — `fast(<1 2>, ...)` /
    `slow(1 2, ...)` apply each factor event's tempo within that event's
    part (innerJoin semantics; `PatternRuntime::FastPattern`/`SlowPattern`,
    `query_tempo_pattern` in value.rs; `extract_tempo_factor_control`,
    builtins.rs); factor values share the constant bounds
    (`positive_rational_tempo_factor`) and are validated per event
  - [x] mini-notation decimal factors — `a*1.5`/`a/1.5` convert through the
    same exact decimal-to-rational path and bounds as `fast`/`slow`
    (`positive_rational_tempo_factor`; `validate_step_tempo_factor`,
    parser.rs; `eval_modified`, eval.rs); `a*0.5` equals `a/2`, and tight
    decimal `*` inside `graph { ... }`/`voice { ... }` stays pedal-DSL
    multiplication
- [x] `rev` — `PatternRuntime::Rev`
- [x] `every` — cycle-localized transforms (`query_transform_cycles`, value.rs)
- [x] `when` — cycle-offset transform (period + offset); real Tidal
  `whenmod(a, b, f, pattern)` also exists and applies `f` on cycles where
  `cycle mod a >= b` (`PatternRuntime::WhenMod`, value.rs)
- [x] `within` — windowed transform
- [x] `jux` — `crates/orpheus-lang/src/builtins.rs` (`apply_jux`)
- [x] `sometimes` — cycle-granularity by design (the whole-cycle transform is
  the intended Orpheus semantic); Tidal's per-event application is spec'd and
  shipped as `sometimes_by(0.5, ...)` — see the `sometimes_by` entry below
- [x] `euclid` — the rotation argument shipped in PR #1393; see the `euclid`
  rotation / `euclidInv` / `euclidFull` entry below
- [x] `stack` / `overlay` — `PatternRuntime::Stack`
- [x] implicit fastcat — whitespace sequences (`eval_sequence`, eval.rs)
- [x] `palindrome` — `apply_palindrome` (builtins.rs)
- [x] `rand` — `PatternRuntime::Rand`
- [x] `chaos` — ≈ Tidal `shuffle`; explicit subdivision counts shipped in
  PR #1390 as `shuffle`/`scramble` — see the `shuffle`/`scramble` entry below
- [x] patternable controls — `gain`/`pan`/`cutoff`/`lpf`/`hpf`/etc. accept
  number patterns (`apply_sample_numeric_control`, builtins.rs)
- [x] control-pattern audit — cycle-varying control arguments (`<a b>`,
  `choose(...)`, `rand`, `irand`) no longer freeze at their cycle-0 value.
  Patterned per cycle (constancy decided structurally via
  `cycle_invariant_constant`; controls sampled cycle by cycle through
  `sample_control_events_by_cycle`, value.rs): `gain`, `pan`, `rate`,
  `pitch`, `transpose`, `lpf`/`hpf`/`cutoff`, `res`, `drive`, `pw`,
  `delay`/`delay_time`/`delay_feedback`, `reverb`/`reverb_room`/
  `reverb_damp`, `chorus`/`chorus_depth`/`chorus_rate`, `compressor`/
  `compressor_threshold`/`compressor_ratio`, `slice`, `slice_idx`, `onset`.
  Constant-by-contract (cycle-varying arguments now raise an explicit
  eval error instead of silently collapsing): counts/periods/offsets/bounds
  (`every`, `when`, `whenmod`, `euclid` pulses/steps/rotation — though the
  inline sugar `bd(<3 5>, 8)` accepts cycle-varying arguments, see the
  grammar section — `run`,
  `scan`, `iter`, `chunk`, `shuffle_slots`, `segment`, `irand`, `rot`,
  `shift`, `off`, `within`, `range` bounds, `strum`, `roll`, `arp` steps,
  `invert`, `drop`, `wolfram`, `lsystem`, `markov`/`wchoose` weights,
  `choose` options, `degrade_by`/`sometimes_by` probabilities, `midi_cc`
  controller) and unit-cycle set builders (`chord` intervals,
  `pitch_class_set`, `tuning` ratios, plugin `notes`/`p` lanes)
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
- [x] `euclid` rotation / `euclidInv` / `euclidFull` — optional third rotation
  argument on `euclid(pulses, steps, rotation)`; `euclid_inv` opens exactly
  the complementary steps; `euclid_full(pulses, steps[, rotation], hits,
  rests)` stacks the hits pattern masked by the gates with the rests pattern
  masked by the inverse (`apply_euclid`/`apply_euclid_full`, builtins.rs)
- [x] `chunk` / `rot` — `PatternRuntime::Chunk`/`Rot` (value.rs),
  `apply_chunk`/`apply_rot` (builtins.rs); `chunk_back` sweeps the parts in
  reverse, `rot` rotates event values while onsets stay put
- [x] `segment` / `range` — sampling continuous patterns into discrete steps;
  `PatternRuntime::Segment`/`Range` (value.rs), `apply_segment`/`apply_range`
  (builtins.rs); bare `rand` is auto-invoked in pattern position so
  `rand |> segment(8) |> range(200, 2000) |> cutoff` works end-to-end
- [x] `run` / `scan` — integer ramp patterns; `run(n)` counts `0..n-1` once
  per cycle (`apply_run`, builtins.rs), `scan(n)` grows the prefix one step
  per cycle and wraps back to `run(1)` after the full ramp, matching Tidal's
  `slowcat $ map run [1 .. n]` (`PatternRuntime::Scan`, value.rs)
- [x] `irand` — integer random source; `PatternRuntime::IRand` (value.rs)
- [x] `choose` / `wchoose` — spec:
  `docs/design/specs/probabilistic_pattern_sequencing_spec.md`; constant
  numeric values only in v1 (`wchoose` takes interleaved `v1, w1, v2, w2, ...`
  pairs); `PatternRuntime::Choose` (value.rs), `apply_choose`/`apply_wchoose`
  (builtins.rs); pattern-valued per-event choice is `pchoose`/`wpchoose`
- [x] `pchoose` / `wpchoose` — pattern-valued per-slot random choice (the
  spec's `choose([bd, sn, cp])` gap; per-event Tidal analogue of
  `randcat`-per-step): each cycle splits into as many equal slots as the
  busiest argument's event count that cycle, and every slot independently
  plays one argument's slice of the slot, chosen at deterministic,
  site-salted random — uniformly, or by interleaved `p1, w1, p2, w2, ...`
  weights sharing the `wchoose`/`wrandcat` validation (negative/all-zero
  weights rejected, zero-weight patterns never drawn). Children keep their
  natural (global) timeline, unlike `randcat`'s localized cycle counters;
  `PatternRuntime::ChooseSlots` (value.rs), `apply_pchoose`/`apply_wpchoose`
  (builtins.rs); remaining: `chooseBy` driven by an external selector
  pattern
- [x] `markov` — spec:
  `docs/design/specs/probabilistic_pattern_sequencing_spec.md`; first-order
  Markov chain over state patterns:
  `markov(s0, w0_0, ..., w0_{k-1}, s1, w1_0, ..., ...)` takes `k` state
  patterns each followed by its `k` outgoing transition weights (`k * (k+1)`
  arguments, no list syntax exists yet — Tidal's `markovPat` list/matrix
  shape is the remaining gap). Cycle 0 plays state 0; each later cycle draws
  the next state from the current state's row at deterministic, site-salted
  random (the walk is replayed per query, so chunked/repeated queries and
  time-travel are stable; negative cycles clamp to the initial state);
  `PatternRuntime::Markov` (value.rs), `apply_markov` (builtins.rs); children
  keep localized (`cycle div n`) cycle counters like `slowcat`
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
- [x] in-sequence commas (inline stacks) — `group_layer` rule (orpheus.pest):
  `(bd sn, hh hh hh)` desugars to a stack of groups squeezed to the group
  span (`build_group`, parser.rs); a bare top-level comma stacks whole lines
  (`binding_lines` rule, `build_binding_lines`)
- [x] inline euclid syntax (`bd(3,8)`) — calls on pattern values desugar to
  `mask(euclid(pulses, steps, rot), token*steps)` (`apply_inline_euclid`,
  builtins.rs; `eval_call_with_args`/`is_inline_euclid_call`, eval.rs)
  - [x] patterned arguments (Tidal `bd("<3 5>", 8)`) — pulses/steps/rotation
    may be cycle-varying number patterns (`bd(<3 5>, 8)`, `bd(3, 8, <0 2>)`):
    each control is sampled once per cycle and cycle `k` gates as
    `euclid(pulses_k, steps_k, rotation_k)`, validated with the constant
    path's rules per cycle (`PatternRuntime::EuclidPattern`/
    `query_euclid_pattern`, value.rs); the standalone `euclid(...)` builtin
    still takes constant arguments

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
- [x] biquad filter node — `biquad(sample_rate_hz, mode)`, RBJ Audio EQ
  Cookbook in transposed direct form II; modes lowpass/highpass/bandpass/
  notch/peaking (peaking adds a gain\_db input channel); coefficients
  recomputed per block from the block-start parameter values
  (`crates/orpheus-dsp/src/graph/filters.rs`); voice-body exposure shipped:
  `eq_peak(x, freq, q, gain_db)` peaking stage with definition-time literal
  range checks (ADR 0010 addendum); shelving modes shipped:
  `BiquadMode::LowShelf`/`HighShelf` (RBJ cookbook, same gain\_db fourth
  input as peaking, same clamps) with voice-body stages
  `eq_low_shelf(x, freq, q, gain_db)` / `eq_high_shelf(...)` lowered onto
  `VoiceNodeSpec::EqShelf`
- [x] SVF (state-variable filter) node — `svf(sample_rate_hz)`, Cytomic/
  Andrew Simper TPT topology, per-sample coefficients so cutoff/Q may sweep
  at audio rate; 3-in (audio, cutoff\_hz, q)/4-out (lowpass, highpass,
  bandpass, notch) split source with exact `lp + bp + hp == input`
  complementarity (`crates/orpheus-dsp/src/graph/filters.rs`); voice-body
  exposure shipped: `svf_lp`/`svf_hp`/`svf_bp`/`svf_notch` stages
  (`(input, cutoff_hz, q)` like `lowpass`) whose cutoff/Q accept bound
  signals for audio-rate sweeps (ADR 0010 addendum)
- [x] ADSR / AR envelope nodes — `adsr`/`ar`
  (`crates/orpheus-dsp/src/graph/primitives.rs`)
- [x] fractional / modulatable delay line — `fdelay(sample_rate_hz,
  max_delay_seconds)`, 2-in (audio, delay\_seconds signal)/1-out, linear
  interpolation, capacity fixed at construction and capped at 10 s
  (`crates/orpheus-dsp/src/graph/primitives.rs`); voice-body exposure shipped:
  a non-literal `delay` time is a signal driving the fractional line at 1 s
  capacity (`delay(x, lfo)`), literal times keep the fixed whole-sample
  `delay_line` (ADR 0010 addendum)
- [x] sample-playback node — `sample_player(&PlaybackSample, sample_rate_hz)`,
  2-in (gate, rate signal)/1-out one-shot player over the bank's shared
  `Arc<[f32]>` mono buffer, resolved at construction; rising gate edge
  restarts, linear-interpolated fractional playhead, ends at the buffer end
  (`crates/orpheus-dsp/src/graph/sample_player.rs`); voice-body exposure
  shipped: `sample("bd"[, rate])` source stage resolves the buffer against
  the session's sample bank at definition time and extends the release tail
  to cover the one-shot (ADR 0010 addendum); follow-ups shipped: loop mode
  (`sample_player_looped` / `sample_loop("bd"[, rate])`, hard-wrap at the
  buffer end, sounds until the release tail ends), pitching by the note
  frequency (`sample_player_pitched` / `sample_pitched("bd"[, reference_hz])`,
  rate = `freq` / reference, default reference 220 Hz — the engine's rate-1.0
  note-frequency convention), and pattern-driven per-note rates via the
  `p1`..`p4` params (`sample("bd", p1)` with `hits |> p1(1 2)`); loop mode
  combined with pitch tracking at the language surface shipped
  (`sample_loop_pitched("bd"[, reference_hz])`, `sample_pitched` validation
  and defaults), and pitched notes below the reference now stretch the
  release tail at trigger time to the playback's true end
  (`max(static, duration x reference / freq)`, capped at 30 s —
  `GraphVoiceSlot::release_frames_for_note`, graph_voice.rs); crossfaded
  loop points shipped (`sample_player_looped_crossfaded`, the
  `loop_crossfade` flag on `VoiceNodeSpec::Sample`, and the
  `sample_loop_xf("bd"[, rate])` / `sample_loop_pitched_xf("bd"[,
  reference_hz])` stages): a short LINEAR (constant-gain — the two reads are
  correlated) fade of 5 ms source material, capped at 10% of the buffer,
  blends the loop tail into the head with no fade buffer, shortening the
  steady-state loop period to `len - fade`; opt-in via a construction flag
  and separate stage names so `sample_loop`'s bit-exact hard-wrap tiling
  stays intact
- [x] `MixNode` adapter — `mix_node` wraps the crossfade in
  `crates/orpheus-dsp/src/synth/mix.rs` (`graph/adapters.rs`)
- [x] `PanNode` — `pan`, 2-in (audio, position)/2-out equal-power panner
  (`crates/orpheus-dsp/src/graph/primitives.rs`)
- [~] engine integration of `graph/` — mostly done (ADR 0009/0010 + addendum):
  pooled graph voices trigger from pattern events via tokens (`gsine`,
  `crates/orpheus-dsp/src/graph_voice.rs`); user-defined programs via the
  `voice { ... }` language block (`crates/orpheus-lang/src/voice.rs`) compile
  to `GraphVoiceSpec` DAGs and hot-swap through
  `EngineCommand::ReplaceGraphVoicePrograms` at cycle boundaries; voice bodies
  now cover the split/merge/feedback topologies — `feedback(body)` with the
  ambient `fb` loop signal (lowered onto `Rec`), `fan(x, branch, ...)`
  split-then-merge (lowered onto `Mrg`), plus `delay`/`gain` stages — and
  per-program polyphony via the `poly = n` pragma binding
  (`GraphVoiceSpec::with_polyphony`, 1..=64) with a `release = s` tail floor;
  voice stealing on pool exhaustion shipped (ADR 0009 addendum): steals the
  most-released, else oldest, voice with a click-free envelope retrigger and
  a 2 ms gain/pan handover ramp, configurable via the `steal = oldest|off`
  pragma (`GraphVoiceSpec::with_steal_policy`, default on);
  pattern-side control signals shipped (ADR 0010 addendum): the ambient
  `p1`..`p4` per-note parameters — set from patterns via the `p1`..`p4`
  controls (`melody |> p1(300 6000)`), stamped as plain f32 fields at trigger
  time and held for the note (unset params read 0), riding the voice
  interface as trailing signal inputs; remaining: smooth per-note parameter
  ramps and audio-rate pattern control of voice parameters
