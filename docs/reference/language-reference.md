# Orpheus Language & Builtin Reference

A complete lookup for every builtin, mini-notation form, `voice{}`/`graph{}`
stage, REPL command, and Orca operator in Orpheus. Signatures and arities in
this document are derived directly from the source of truth
(`crates/orpheus-lang/src/builtins.rs`) and are guarded against drift by
`crates/orpheus-lang/tests/reference_builtins_sync.rs`, which fails the build if
the builtin list here and the registry ever disagree.

---

## 1. How to read this

- **Notation.** Signatures are written `name(arg1, arg2, …, pattern)`. The final
  argument is (almost always) the pattern the transform acts on.
- **Auto-currying.** Every builtin is a curried function. Supplying fewer than
  its arity's worth of arguments returns a partially-applied function; supplying
  the last argument runs it. Because the pattern is the last argument, the two
  forms below are equivalent, and the piped form is the idiomatic one:

  ```text
  drums = fast(2, bd sn)      # direct call
  drums = bd sn |> fast(2)    # piped — fast(2) is applied to `bd sn`
  ```

- **`|>` pipe.** Left-to-right application: `x |> f(a)` means `f(a, x)`. Chains
  compose left to right: `bd sn |> fast(2) |> rev`.
- **Arity.** The number of arguments a builtin consumes before it executes.
  *Variadic* builtins (marked ⁺) accept **more** than their base arity — e.g.
  `cat` takes 2 or more patterns.
- **Value builtins.** A few names resolve to *values*, not functions: the sample
  atoms (`bd`…), the scale sets (`ionian`…), and the arp-direction atoms
  (`up`, `down`, `pingpong`). They have no arity.
- **Loose vs Strict mode.** The REPL evaluates lines in **Loose** mode
  (permissive type inference, good for improvisation). `.ode` script files are
  evaluated in **Strict** mode (full checking). Every example here that is shown
  as a bare `name = …` binding evaluates in Loose mode unless noted; the
  `voice{}`/`graph{}` examples use Strict mode.
- **Cycle.** The unit of musical time. One cycle = 4 beats. A bare
  whitespace-separated sequence fills exactly one cycle.

---

## 2. Mini-notation

Orpheus has a real grammar (`crates/orpheus-lang/src/grammar/orpheus.pest`), so
pattern notation is written as bare source — there are no quoted mini-notation
strings. Every form below is a grammar rule (except inline euclid, which is
desugared in `builtins.rs`).

| Form | Grammar rule | Meaning | Example |
|---|---|---|---|
| `a b c` | `sequence` | Whitespace sequence (fastcat): events share one cycle | `drums = bd sn cp sn` |
| `~` | `rest` | A silent step | `drums = bd ~ sn ~` |
| `( … )` | `group` | Subdivision group: squeezes its contents into one step | `drums = bd (sn cp)` |
| `( a , b )` | `group_layer` | In-group stack: layers play simultaneously in the step | `drums = (bd sn, hh hh hh)` |
| `a = x, y` | `binding_lines` | Top-level comma stacks whole lines | `drums = bd sn, hh*4` |
| `<a b c>` | `alternation` | Alternation: one element per cycle, cycling | `drums = <bd sn cp>` |
| `a*n` | `repeat_modifier` | Speed up / repeat a step n times within its slot | `drums = bd*2` |
| `a/n` | `slow_modifier` | Slow a step down by n | `drums = (bd sn)/2` |
| `a!n` | `replicate_modifier` | Replicate into n separate steps | `drums = bd!3 sn` |
| `a?` / `a?p` | `degrade_modifier` | Randomly drop the step (optional probability p) | `drums = (bd sn cp hh)?` |
| `{a b, c d e}` | `polymeter` | Polymeter: subsequences share a step count | `drums = {bd sn, hh hh hh}` |
| `{…}%n` | `polymeter_steps` | Override the polymeter step count to n | `drums = {bd sn cp hh}%2` |
| `1.5`, `-2` | `number` | Decimal and negative numbers | `swing = shift(-0.5, 60)` |
| `"text"` | `string` | String literal (escapes `\" \\ \n \r \t`) | `kit = sample("bd")` |
| `x + y` | `sum_expr` | Spaced binary addition (distinct from step `*`) | `m = 0*1.5 1` |
| `x * y` | `product_expr` | Spaced binary product (distinct from tight `bd*2`) | `bd * 2` |
| `x \|> f` | `pipe_expr` | Pipe: apply `f` to `x` | `hats = hh*8 \|> gain(0.6)` |
| `bd(3,8)` | *inline euclid*¹ | Euclidean rhythm: 3 onsets across 8 steps | `drums = bd(3, 8)` |
| `bd(3,8,r)` | *inline euclid*¹ | …with rotation `r` | `drums = bd(3, 8, 1)` |

¹ **Inline euclid is not in the pest grammar.** `bd(3,8)` is parsed as a call
suffix on a pattern value and desugared by `apply_inline_euclid` in
`builtins.rs` to `mask(euclid(3, 8), bd*8)`. Its arguments may be cycle-varying
patterns: `drums = bd(<3 8>, 8)`.

Note the difference between the **tight** step operator and the **spaced**
binary operator: `bd*2` (no spaces) is a step-repeat modifier, while `bd * 2`
(spaces) is a binary product.

---

## 3. Source atoms

These resolve to a value, not a function.

- **Sample atoms** (each a one-event sample pattern): `bd` `sn` `cp` `hh` `saw`
  `pulse` `tri` `noise`. Example: `hats = hh`.
- **Named pitch literals** (parsed in `pitch.rs`): lowercase note + optional
  accidental (`s` sharp / `f` flat) + octave, e.g. `c4` (MIDI 60), `fs4`, `bf3`.
  Example: `melody = c4 e4 g4`.
- **Scales** (pitch-class sets): `ionian` `dorian` `phrygian` `mixolydian`
  `aeolian` `minor_pentatonic`. Example: `line = degrees(aeolian, 0 2 4 7 8)`.
- **Arp directions** (atoms consumed by `arp`): `up` `down` `pingpong`
  (`updown` is an alias for `pingpong`). Example: `lead = arp(5, up, chord(c4, 0 4 7))`.

---

## 4. Builtins

Every builtin below resolves through `builtin_value(name)`. Arities are taken
verbatim from `BuiltinKind::arity`. ⁺ marks variadic builtins (accept more than
the base arity).

### 4.1 Time

| Builtin | Arity | Signature | Semantics | Example |
|---|---|---|---|---|
| `fast` | 2 | `fast(factor, pat)` | Speed the pattern up by `factor` (factor may itself be a pattern) | `drums = bd sn \|> fast(2)` |
| `slow` | 2 | `slow(factor, pat)` | Stretch the pattern by `factor` | `drums = bd sn \|> slow(2)` |
| `rev` | 1 | `rev(pat)` | Reverse events within each cycle | `drums = bd sn cp sn \|> rev` |
| `shift` | 2 | `shift(amount, pat)` | Rotate events forward by `amount` cycles | `drums = bd sn \|> shift(0.25)` |
| `palindrome` | 1 | `palindrome(pat)` | Alternate forward and reversed cycles | `drums = palindrome(bd sn)` |
| `iter` | 2 | `iter(n, pat)` | Rotate the sequence left by one step each cycle, over `n` cycles | `drums = iter(4, bd sn cp hh)` |
| `iter_back` | 2 | `iter_back(n, pat)` | Like `iter` but rotating right | `drums = iter_back(4, bd sn cp hh)` |

### 4.2 Cycle-scoped

| Builtin | Arity | Signature | Semantics | Example |
|---|---|---|---|---|
| `every` | 3 | `every(n, transform, pat)` | Apply `transform` on every `n`-th cycle | `drums = bd sn \|> every(2, fast(2))` |
| `when` | 4 | `when(modulo, remainder, transform, pat)` | Apply `transform` on cycles where `cycle % modulo == remainder` | `drums = bd sn \|> when(3, 1, rev)` |
| `whenmod` | 4 | `whenmod(modulo, threshold, transform, pat)` | Apply `transform` on cycles where `cycle % modulo >= threshold` | `drums = bd sn \|> whenmod(4, 2, rev)` |
| `within` | 4 | `within(start, end, transform, pat)` | Apply `transform` only to the `[start, end)` slice of each cycle | `drums = bd sn cp hh \|> within(0, 0.5, rev)` |

### 4.3 Probabilistic

All randomness is deterministic (onset-hash + per-site salt), so a pattern
renders identically every run.

| Builtin | Arity | Signature | Semantics | Example |
|---|---|---|---|---|
| `degrade` | 1 | `degrade(pat)` | Randomly drop ~50% of events | `m = 0 1 2 3 4 5 6 7 \|> degrade` |
| `degrade_by` | 2 | `degrade_by(prob, pat)` | Randomly drop events with probability `prob` | `drums = bd sn \|> degrade_by(0.25)` |
| `sometimes` | 2 | `sometimes(transform, pat)` | Apply `transform` to ~50% of events | `drums = bd sn \|> sometimes(fast(2))` |
| `sometimes_by` | 3 | `sometimes_by(prob, transform, pat)` | Apply `transform` to events with probability `prob` | `drums = bd sn \|> sometimes_by(0.5, rev)` |
| `often` | 2 | `often(transform, pat)` | `sometimes_by` at p=0.75 | `drums = often(rev, bd sn)` |
| `rarely` | 2 | `rarely(transform, pat)` | `sometimes_by` at p=0.25 | `drums = rarely(rev, bd sn)` |
| `almost_always` | 2 | `almost_always(transform, pat)` | `sometimes_by` at p=0.9 | `drums = almost_always(rev, bd sn)` |
| `almost_never` | 2 | `almost_never(transform, pat)` | `sometimes_by` at p=0.1 | `drums = almost_never(rev, bd sn)` |
| `chaos` | 1 | `chaos(pat)` | Modulate events with a logistic-map chaotic signal | `a = chaos(bd sn cp hh)` |
| `rand` | 0 | `rand()` | Continuous random signal in `[0, 1)` | `r = rand()` |
| `irand` | 1 | `irand(n)` | Random integers in `[0, n)` | `m = irand(8)` |
| `choose` ⁺ | 2 | `choose(a, b, …)` | Pick one of the values per cycle | `m = choose(1, 2, 3, 4)` |
| `wchoose` ⁺ | 4 | `wchoose(val, weight, …)` | Weighted `choose` over value/weight pairs | `m = wchoose(1, 1, 2, 3)` |
| `pchoose` ⁺ | 2 | `pchoose(a, b, …)` | Per-cycle choice among whole patterns | `m = pchoose(0 1, 2 3)` |
| `wpchoose` ⁺ | 4 | `wpchoose(pat, weight, …)` | Weighted `pchoose` over pattern/weight pairs | `drums = wpchoose(bd, 1, sn, 3)` |
| `randcat` ⁺ | 2 | `randcat(a, b, …)` | Concatenate patterns in random cycle order | `m = randcat(0 1, 2 3)` |
| `wrandcat` ⁺ | 4 | `wrandcat(pat, weight, …)` | Weighted `randcat` over pattern/weight pairs | `drums = wrandcat(bd, 1, sn, 3)` |
| `markov` ⁺ | 6 | `markov(s0, w00, w01, s1, w10, w11, …)` | Markov chain over states with transition weights | `m = markov(0 1, 1, 2, 2 3, 3, 1)` |
| `shuffle` | 2 | `shuffle(n, pat)` | Deterministically permute `n` slots per cycle | `m = 10 20 30 40 \|> shuffle(4)` |
| `scramble` | 2 | `scramble(n, pat)` | Like `shuffle` but with replacement | `m = 10 20 30 40 \|> scramble(4)` |

### 4.4 Concatenation & rotation

| Builtin | Arity | Signature | Semantics | Example |
|---|---|---|---|---|
| `cat` / `slowcat` ⁺ | 2 | `cat(a, b, …)` | Concatenate patterns, one per cycle | `drums = cat(bd, sn cp)` |
| `append` | 2 | `append(a, b)` | Concatenate two patterns, one per cycle | `drums = append(bd, sn)` |
| `off` | 3 | `off(amount, transform, pat)` | Overlay a copy shifted by `amount` and transformed | `drums = off(0.25, rev, bd sn)` |
| `rot` | 2 | `rot(n, pat)` | Rotate event **values** by `n` while keeping timing | `drums = rot(1, bd sn cp)` |
| `chunk` | 3 | `chunk(n, transform, pat)` | Apply `transform` to a different 1/`n` chunk each cycle | `drums = chunk(4, rev, bd sn cp hh)` |
| `chunk_back` | 3 | `chunk_back(n, transform, pat)` | Like `chunk` but iterating backwards | `drums = chunk_back(4, rev, bd sn cp hh)` |

### 4.5 Sampling continuous patterns

| Builtin | Arity | Signature | Semantics | Example |
|---|---|---|---|---|
| `segment` | 2 | `segment(n, pat)` | Sample a continuous signal into `n` discrete events per cycle | `m = rand() \|> segment(4)` |
| `range` | 3 | `range(min, max, pat)` | Scale a `[0,1]` signal into `[min, max]` | `m = range(200, 2000, 0 0.5 1)` |
| `run` | 1 | `run(n)` | Ascending integer run `0..n` across one cycle | `ramp = run(4)` |
| `scan` | 1 | `scan(n)` | Growing runs: cycle `k` plays `0..k` up to `n` | `ramp = scan(3)` |

### 4.6 Euclidean family

| Builtin | Arity | Signature | Semantics | Example |
|---|---|---|---|---|
| `euclid` ⁺ | 2 | `euclid(pulses, steps[, rotation])` | Boolean Euclidean rhythm mask | `drums = mask(euclid(3, 8), bd*8)` |
| `euclid_inv` ⁺ | 2 | `euclid_inv(pulses, steps[, rotation])` | Inverted Euclidean mask | `drums = mask(euclid_inv(3, 8), bd*8)` |
| `euclid_full` ⁺ | 4 | `euclid_full(pulses, steps, onPat, offPat)`² | Euclidean placement of `onPat`, filling gaps with `offPat` | `drums = euclid_full(3, 8, bd*8, sn*8)` |

² `euclid_full` accepts an optional `rotation` inserted as the third argument:
`euclid_full(3, 8, 1, bd*8, sn*8)`.

### 4.7 Layering

| Builtin | Arity | Signature | Semantics | Example |
|---|---|---|---|---|
| `jux` | 2 | `jux(transform, pat)` | Play `pat` panned left and `transform(pat)` panned right | `a = jux(rev, bd sn)` |
| `through` | 2 | `through(pedal, pat)` | Route a sample pattern through a `graph{}` pedal | `lead = through(drivebox, saw)` |

For simple simultaneous layering use the grammar's `stack(...)` /
comma-stacking (§2), e.g. `drums = stack(bd sn, hh*4)`.

### 4.8 Pitch & harmony

| Builtin | Arity | Signature | Semantics | Example |
|---|---|---|---|---|
| `chord` | 2 | `chord(root, degrees)` | Build a chord from a root note and degree offsets | `pad = chord(c4, 0 4 7)` |
| `invert` | 2 | `invert(n, chordPat)` | Raise the lowest `n` chord tones an octave (inversion) | `pad = invert(1, chord(c4, 0 4 7))` |
| `drop` | 2 | `drop(n, chordPat)` | Drop-`n` voicing: lower the `n`-th voice from the top | `pad = drop(2, chord(c4, 0 4 7 10))` |
| `degrees` | 2 | `degrees(scale, pat)` | Map scale-degree indices onto a scale/pitch-class set | `line = degrees(aeolian, 0 2 4 7 8)` |
| `pitch_class_set` | 1 | `pitch_class_set(pat)` | Build a pitch-class set from a number pattern | `pcs = pitch_class_set(0 4 7)` |
| `transpose` | 2 | `transpose(semitones, pat)` | Semitone shift; interchangeable with `pitch`: adds to note numbers, shifts sample/voice playback pitch | `melody = transpose(12, c4 e4 g4)` |
| `pitch` | 2 | `pitch(semitones, pat)` | Semitone shift; interchangeable with `transpose`: adds to note numbers, shifts sample/voice playback pitch | `drums = bd \|> pitch(7)` |
| `strum` | 1 | `strum(chordPat)` | Spread chord tones across time (strummed) | `pad = strum(chord(c4, 0 4 7))` |
| `roll` | 2 | `roll(count, pat)` | Ratchet each event into `count` rapid repeats | `buzz = roll(4, sn)` |
| `arp` | 3 | `arp(count, dir, chordPat)` | Arpeggiate a chord into `count` notes in direction `dir` | `lead = arp(5, up, chord(c4, 0 4 7))` |
| `notes` | 2 | `notes(notePat, plugin)` | Attach a note pattern to a hosted plugin instrument | `lead = vst("Massive") \|> notes(c4 e4)` |

### 4.9 Tuning / microtonal

| Builtin | Arity | Signature | Semantics | Example |
|---|---|---|---|---|
| `tuning` | 1 | `tuning(ratios)` | Build a tuning from a cycle-invariant ratio list | `t = tuning(1.0 1.125 1.25 1.5 2.0)` |
| `load_scl` | 1 | `load_scl(path)` | Load a Scala `.scl` file into a tuning | `t = load_scl("scl/young.scl")` |
| `tune` | 2 | `tune(tuning, pat)` | Retune a sample pattern with a tuning | `drums = tune(tuning(1.0 1.5 2.0), bd sn)` |

### 4.10 Generative

| Builtin | Arity | Signature | Semantics | Example |
|---|---|---|---|---|
| `lsystem` | 3 | `lsystem(axiom, iterations, rules)` | Expand an L-system into a number pattern | `pat = lsystem("A", 3, "A:AB,B:A")` |
| `wolfram` | 2 | `wolfram(rule, generations)` | Elementary cellular-automaton pattern | `w = wolfram(30, 3)` |

### 4.11 Controls & effects

These tag sample-pattern events with control values (curry with the pattern
last). Values may be constants or patterns.

| Builtin | Arity | Signature | Semantics | Example |
|---|---|---|---|---|
| `gain` | 2 | `gain(amount, pat)` | Scale event amplitude | `drums = bd \|> gain(0.8)` |
| `pan` | 2 | `pan(pos, pat)` | Stereo pan, `-1`..`1` | `lead = saw \|> pan(-1)` |
| `cutoff` | 2 | `cutoff(hz, pat)` | Filter cutoff frequency | `drums = bd \|> cutoff(800)` |
| `res` | 2 | `res(q, pat)` | Filter resonance | `drums = bd \|> res(0.5)` |
| `hpf` | 2 | `hpf(hz, pat)` | High-pass filter cutoff | `drums = bd \|> hpf(200)` |
| `lpf` | 2 | `lpf(hz, pat)` | Low-pass filter cutoff | `drums = bd \|> lpf(2000)` |
| `drive` | 2 | `drive(amount, pat)` | Waveshaping drive | `drums = bd \|> drive(1.5)` |
| `pw` | 2 | `pw(width, pat)` | Pulse width | `lead = pulse \|> pw(0.3)` |
| `delay` | 2 | `delay(mix, pat)` | Delay send amount | `drums = bd \|> delay(0.3)` |
| `delay_time` | 2 | `delay_time(seconds, pat)` | Delay time | `drums = bd \|> delay_time(0.25)` |
| `delay_feedback` | 2 | `delay_feedback(amount, pat)` | Delay feedback | `drums = bd \|> delay_feedback(0.4)` |
| `reverb` | 2 | `reverb(mix, pat)` | Reverb send amount | `drums = bd \|> reverb(0.3)` |
| `reverb_room` | 2 | `reverb_room(size, pat)` | Reverb room size | `drums = bd \|> reverb_room(0.6)` |
| `reverb_damp` | 2 | `reverb_damp(amount, pat)` | Reverb damping | `drums = bd \|> reverb_damp(0.5)` |
| `chorus` | 2 | `chorus(mix, pat)` | Chorus send amount | `lead = saw \|> chorus(0.4)` |
| `chorus_depth` | 2 | `chorus_depth(amount, pat)` | Chorus depth | `lead = saw \|> chorus_depth(0.5)` |
| `chorus_rate` | 2 | `chorus_rate(hz, pat)` | Chorus LFO rate | `lead = saw \|> chorus_rate(1.5)` |
| `compressor` | 2 | `compressor(amount, pat)` | Compression amount | `drums = bd \|> compressor(0.5)` |
| `compressor_threshold` | 2 | `compressor_threshold(v, pat)` | Compressor threshold | `drums = bd \|> compressor_threshold(0.4)` |
| `compressor_ratio` | 2 | `compressor_ratio(v, pat)` | Compressor ratio | `drums = bd \|> compressor_ratio(4)` |
| `onset` | 2 | `onset(index, pat)` | Sample start offset (onset index) | `drums = bd \|> onset(0.25)` |
| `rate` | 2 | `rate(rate, pat)` | Sample playback rate | `drums = bd \|> rate(1.5)` |
| `sample` | 1 | `sample(token)` | Resolve a sample by string token | `kit = sample("bd")` |
| `slice` | 3 | `slice(start, end, pat)` | Play the `[start, end)` fraction of the sample | `drums = bd \|> slice(0.25, 1)` |
| `slice_idx` | 3 | `slice_idx(index, segments, pat)` | Play the `index`-th of `segments` equal slices | `drums = bd \|> slice_idx(1, 4)` |
| `cc` / `midi_cc` | 1 | `cc(controller)` | Normalized live value of MIDI CC `controller` | `mod = cc(74)` |
| `p` / `param` | 3 | `p(name, control, plugin)` | Set a hosted-plugin parameter | `lead = vst("Massive") \|> p("cutoff", 0.5)` |
| `p1` | 2 | `p1(value, pat)` | Set per-note voice param 1 | `lead = bd sn \|> p1(300 4000)` |
| `p2` | 2 | `p2(value, pat)` | Set per-note voice param 2 | `lead = bd \|> p2(0.5)` |
| `p3` | 2 | `p3(value, pat)` | Set per-note voice param 3 | `lead = bd sn \|> p3(7)` |
| `p4` | 2 | `p4(value, pat)` | Set per-note voice param 4 | `lead = bd \|> p4(0.2)` |
| `vst` | 1 | `vst(name)` | Load a VST3 plugin instrument | `lead = vst("Massive")` |
| `au` | 1 | `au(name)` | Load an AudioUnit plugin instrument | `lead = au("DLSMusicDevice")` |
| `hex` | 1 | `hex(string)` | Bit pattern from a hex string (4 bits/char) | `m = hex("a")` |
| `bin` | 1 | `bin(string)` | Bit pattern from a binary string | `m = bin("1010")` |

---

## 5. The `voice{}` / `graph{}` instrument DSL

`voice{}` defines an instrument; `graph{}` is the raw signal-graph form sharing
the same body grammar (semicolon-separated `name = expr` bindings ending in one
result expression). A voice compiles to a DAG hot-swapped into the engine at
cycle boundaries; the binding name becomes a pattern token.

```text
pluck = voice { osc = saw(freq) ; env = adsr(gate, 0.001, 0.02, 0.5, 0.05) ; osc * env }
melody = c4 e4 g4 |> pluck
```

### 5.1 Ambient signals (in scope inside a voice body)

- `gate` — 1 while the triggering event span is held, 0 otherwise.
- `freq` — note frequency in Hz (`ref_freq × event rate`).
- `fb` — the feedback loop signal; only valid inside `feedback(…)`.
- `p1`..`p4` — per-note parameters fed from the pattern side (e.g.
  `melody |> p1(<200 800>)`), sampled at trigger and held for the note (0 if unset).

### 5.2 Stages (dispatched in `voice.rs`)

- **Oscillators:** `sine(freq)`, `saw(freq)`, `tri(freq)`, `pulse(freq[, width])`,
  `noise()`.
- **Envelopes:** `adsr(gate, a, d, s, r)`, `ar(gate, a, r)` — envelope params must
  be literals.
- **Filters:** `lowpass(x, cutoff, q)`; state-variable `svf_lp` / `svf_hp` /
  `svf_bp` / `svf_notch(x, cutoff, q)` (cutoff/q may be bound signals for
  audio-rate sweeps); EQ `eq_peak(x, freq, q, gain_db)`,
  `eq_low_shelf(x, freq, q, gain_db)`, `eq_high_shelf(x, freq, q, gain_db)`.
- **Shaping / mix:** `drive(x, amount)`, `gain(x, amount)`, `delay(x, time)`
  (literal time = fixed delay line, ≤ 30 s; signal time = fractional line),
  and `+` / `*` for arithmetic mixing.
- **Topology:** `feedback(body)` (uses ambient `fb`, lowers to a recursive `Rec`
  node), `fan(x, branch, …)` (split-then-merge).
- **Sample sources:** `sample("bd"[, rate])`, `sample_loop`, `sample_loop_xf`,
  `sample_pitched`, `sample_loop_pitched`, `sample_loop_pitched_xf`.

Verified examples:

```text
acid = voice { body = saw(freq) + tri(freq) * 0.5 ; shaped = body |> lowpass(1200, 0.3) |> drive(1.5) ; shaped * ar(gate, 0.001, 0.08) }
echo = voice { release = 0.5 ; dry = sine(freq) * ar(gate, 0.001, 0.01) ; wet = feedback(dry + fb |> delay(0.05) |> gain(0.5)) ; dry + wet }
bank = voice { osc = saw(freq) ; fan(osc, lowpass(500, 0.2), lowpass(3000, 0.2)) * ar(gate, 0.001, 0.05) }
kit = voice { sample("bd") }
```

### 5.3 Pragmas (voice-scoped configuration bindings)

| Pragma | Range | Meaning |
|---|---|---|
| `poly = n` | 1..=64 | Pooled polyphony (voice count) |
| `release = seconds` | ≤ 30 s | Release-tail floor |
| `steal = oldest \| off` | — | Voice-stealing policy on pool exhaustion |
| `param_ramp = seconds` | 0..=1 s | Glide time for `p1`–`p4` on steal/retrigger |

```text
lead = voice { poly = 4 ; sine(freq) * ar(gate, 0.001, 0.05) }
```

### 5.4 Built-in graph voice token

`gsine` is a ready-made pooled graph voice (gated sine → envelope → gain → pan),
registered by `builtin_graph_voice_programs()` in
`crates/orpheus-dsp/src/graph_voice.rs`. Use it as a pattern token like any
sample.

---

## 6. Graph / DSP primitives (ADR 0004)

The Faust-style block-diagram algebra lives in `crates/orpheus-dsp/src/graph/`
and is re-exported from `graph/mod.rs`. `voice{}`/`graph{}` blocks lower onto
these nodes.

- **Trait / error:** `Node`, `GraphError`.
- **Five combinators:** `seq`/`Seq` (sequential), `par`/`Par` (parallel),
  `split`/`Spl`, `merge`/`Mrg`, `feedback`/`Rec` (recursive).
- **Composition helpers:** `Processor`, `pipe`, `bind`/`Bind`.
- **Primitives:** `sine`, `adsr`/`AdsrNode`, `ar`/`ArNode`, `one_pole`,
  `delay_line`/`DelayNode`, `fdelay`/`FractionalDelayNode` (≤ 10 s),
  `pan`/`PanNode` (equal-power), `sum`, `wire`, `constant`, `passthrough`.
- **Filters:** `svf`/`SvfNode` (Cytomic TPT state-variable, per-sample coeffs,
  lp/hp/bp/notch outputs); `biquad`/`BiquadNode` (RBJ cookbook) with `BiquadMode`
  ∈ lowpass/highpass/bandpass/notch/peaking/lowshelf/highshelf.
- **Adapters (over `synth/`):** `saw`, `pulse`, `tri`, `noise`, `gain_node`,
  `ladder_filter`, `mix_node`, `soft_sat`.
- **Sample playback:** `sample_player` (one-shot), `sample_player_looped`,
  `sample_player_looped_crossfaded`, `sample_player_pitched`.

Parameters flow as signal inputs (Faust model): a filter's cutoff is an input
channel, not a config field. Channel counts are validated at construction time.

---

## 7. REPL commands

Entered at the `> ` prompt; anything not starting with `:` is evaluated as a
binding. Source: `build_help_table` in `session.rs`.

| Command | Description |
|---|---|
| `:env` | List all bindings in the environment |
| `:undo` | Restore the previous session state |
| `:redo` | Reapply the most recently undone state |
| `:explain <binding>` | Explain a pattern/pedal's internal structure |
| `:stats <binding>` | Show event density and statistics |
| `:render <binding> <path> <cycles>` | Render to an audio file (`.wav`/`.flac`) |
| `:export <binding> <path> <cycles>` | Export to a format (MIDI, SVG, …) |
| `:export stems <binding> <dir> <cycles>` | Export per-track stems to a directory |
| `:roll <binding>` | Display an ASCII piano roll |
| `:tempo <bpm>` | Set global tempo (beats per minute) |
| `:ref_freq <hz>` | Set global reference frequency for tuning |
| `:samples <dir>` | Load additional samples from a directory |
| `:import stems <dir>` | Load stem WAVs as patterns and routed tracks |
| `:reload-samples` | Reload the most recently loaded sample directory |
| `:open <path>` | Open and evaluate an external `.ode` script |
| `:track …` | Manage mixer tracks (new, bind, level, mute) |
| `:bus …` | Manage mixer buses and effects (new, fx) |
| `:send …` | Manage track effect sends to buses |
| `:mixer` | Display tracks, buses, and sends |
| `:midi …` | Manage MIDI inputs and outputs |
| `:play` | Start the transport clock |
| `:stop` | Stop the transport clock |
| `:help` | List REPL commands |
| `:quit` | Exit the session |

The TUI additionally provides the `:orca …` command family (`:orca udp/osc/midi/listen …`)
to configure the Orca surface's transports.

---

## 8. Orca operators

Orpheus embeds an [Orca](https://100r.co/site/orca.html)-style 2D grid surface
(`crates/orpheus-lang/src/orca/`). Each frame ticks the grid; operators rewrite
cells and IO operators emit events. Frame `N` maps to `TimeSpan [N/F, (N+1)/F)`.
See `docs/design/orca-surface.md` for the full surface.

**Pure operators (A–Z):**

| Op | Name | Op | Name | Op | Name |
|---|---|---|---|---|---|
| `N` | move north | `I` | increment | `S` | move south |
| `E` | move east | `W` | move west | `A` | add |
| `B` | subtract | `C` | clock | `D` | delay |
| `F` | if (equality) | `G` | generate | `H` | halt |
| `J` | jumper (vertical) | `K` | konkat | `L` | lesser |
| `M` | multiply | `O` | read (offset) | `P` | push |
| `Q` | query | `R` | random | `T` | track |
| `U` | euclid | `V` | variable | `X` | write (offset) |
| `Y` | jymper (horizontal) | `Z` | lerp | | |

**IO operators:**

| Glyph | Meaning |
|---|---|
| `:` | MIDI note out |
| `%` | monophonic MIDI note out |
| `!` | MIDI CC out |
| `?` | MIDI pitch-bend out |
| `;` | UDP send |
| `=` | OSC send |
| `$` | self / command interpreter |
| `#` | comment (row toggle) |

---

## Appendix: Registry index (machine-checked)

Every name below resolves through `builtin_value`. The drift-guard test
`reference_builtins_sync.rs` parses this block and asserts it is exactly the set
the registry knows — no more, no less. Keep it in sync when adding or removing a
builtin.

<!-- REGISTRY-INDEX-START -->
`bd` `sn` `cp` `hh` `saw` `pulse` `tri` `noise`
`ionian` `dorian` `phrygian` `mixolydian` `aeolian` `minor_pentatonic`
`up` `down` `pingpong` `updown`
`fast` `slow` `rev` `shift` `palindrome` `iter` `iter_back`
`every` `when` `whenmod` `within`
`degrade` `degrade_by` `sometimes` `sometimes_by` `often` `rarely` `almost_always` `almost_never` `chaos` `rand` `irand` `choose` `wchoose` `pchoose` `wpchoose` `randcat` `wrandcat` `markov` `shuffle` `scramble`
`cat` `slowcat` `append` `off` `rot` `chunk` `chunk_back`
`segment` `range` `run` `scan`
`euclid` `euclid_inv` `euclid_full`
`jux` `through`
`chord` `invert` `drop` `degrees` `pitch_class_set` `transpose` `pitch` `strum` `roll` `arp` `notes`
`tuning` `load_scl` `tune`
`lsystem` `wolfram`
`gain` `pan` `cutoff` `res` `hpf` `lpf` `drive` `pw` `delay` `delay_time` `delay_feedback` `reverb` `reverb_room` `reverb_damp` `chorus` `chorus_depth` `chorus_rate` `compressor` `compressor_threshold` `compressor_ratio` `onset` `rate` `sample` `slice` `slice_idx` `cc` `midi_cc` `p` `param` `p1` `p2` `p3` `p4` `vst` `au` `hex` `bin`
<!-- REGISTRY-INDEX-END -->
