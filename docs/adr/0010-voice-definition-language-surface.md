# ADR 0010: Language Surface for User-Defined Graph Voice Programs

- Status: Accepted
- Date: 2026-07-08
- Extends: ADR 0009 (graph voice engine integration), ADR 0004 (graph
  combinators)

## Context

ADR 0009 wired graph-compiled voices into the render engine behind a static
built-in program table (`gsine`) and named two follow-ups: a user-facing way
to define programs from the language, and an
`EngineCommand::ReplaceGraphVoicePrograms` swap mirroring `ReplaceSampleBank`.
Until now a user could not define an instrument: the sonic surface of graph
voices was exactly one hardcoded program.

Two surface shapes were considered:

- a `synth(...)`-style builtin taking positional/named arguments — zero
  grammar changes, but named arguments do not survive general evaluation
  (assignment expressions are pedal-DSL-only), and a flat argument list cannot
  express signal routing (an envelope sweeping a filter cutoff);
- a block form following the pedal DSL precedent (`graph { ... }`), which
  already established how Orpheus describes DSP in-language: let-bound
  signals, stage calls, `|>` pipes, and `+`/`*` arithmetic.

## Decision

**`name = voice { ... }` block, sharing the pedal DSL's body grammar.** The
`voice` block reuses the existing `graph_body` grammar rule (bindings then one
result expression) and the pedal DSL's tight-`*`-is-multiplication rewrite, so
the parser change is one rule plus one AST variant. The binding name is the
program's pattern token.

**Voice-specific compilation, not pedal compilation.** A voice body sees two
ambient inputs — `gate` (1 during the event span) and `freq` (note Hz) — and
composes the graph vocabulary that `orpheus-dsp` exports: `sine`/`saw`/`tri`/
`pulse`/`noise` oscillators, gate-driven `adsr`/`ar` envelopes (segment times
as literals, bound as constants per ADR 0004's `bind`), the ladder `lowpass`,
`drive` saturation, and `+`/`*` mixing. The result is the mono voice signal;
the per-trigger gain and equal-power pan stages are appended automatically to
meet ADR 0009's fixed `[gate, freq, gain, pan] -> [left, right]` interface.
The program's release tail is the longest envelope release in the body (small
default when none), preserving ADR 0009's deterministic voice end.

**Declarative spec as the crate boundary.** Compilation produces a
`GraphVoiceSpec` (`crates/orpheus-dsp/src/graph_voice.rs`): a flat, validated
DAG of data-only `VoiceNodeSpec`s. Specs are `Clone`/`PartialEq`, so they can
ride inside `EngineCommand` and give banks structural equality. Lowering onto
combinators is generic: each node becomes a `wire`-then-`par` stage that
prepends its output to a growing signal bus, reusing `seq`/`par`/`wire`/
`passthrough` rather than adding a new evaluator. Validation at `new()` means
compilation cannot fail later.

**Swap discipline mirrors the sample bank.** The session rebuilds a complete
`GraphVoiceBank` (built-ins plus one program per `voice` binding, user
programs shadowing same-token built-ins) on the language thread —
construction compiles and pre-warms every pooled voice — and enqueues
`EngineCommand::ReplaceGraphVoicePrograms`. The audio thread stores it as
pending and moves it into place at the next cycle boundary, exactly like
`ReplaceSampleBank`; redefining a voice mid-session is therefore safe and
takes effect on the next cycle. `Clone` on a bank rebuilds from the retained
specs (documented as off-thread-only); `PartialEq` compares specs and sample
rate.

**Token resolution keeps sample-bank precedence.** A voice binding's name
resolves inside pattern sequences as a sample-like token
(`Pattern<Sample>`-coercing in the type system); at trigger time the sample
bank and built-in fallbacks keep first claim, and only unresolved tokens reach
the graph bank (ADR 0009 unchanged).

## Consequences

- Users define instruments in session files, e.g.
  `pluck = voice { osc = saw(freq) ; env = adsr(gate, 0.001, 0.02, 0.5, 0.05) ; osc * env }`
  and play them with `melody = pluck ~ pluck ~`.
- The audio-thread path stays allocation-free: the counting-allocator test
  covers a spec-compiled user voice alongside the built-in.
- Notes sounding on the outgoing pool end at the swap boundary when a bank is
  replaced; the old bank is dropped on the audio thread, the same trade-off
  the sample-bank swap already accepts.
- The voice vocabulary is a curated subset of the graph module: `split`/
  `merge`/`feedback` topologies, audio-rate envelope-segment modulation, and
  per-program polyphony configuration are deliberate follow-ups. (The first
  and last landed; see the addendum below.)
- A bare voice binding referenced outside a pattern is the voice value itself
  (aliasing), not a pattern; tokens only take effect inside sequences.

## Addendum: feedback/fan topologies and per-program polyphony

The deferred topology and polyphony items shipped as an extension of the same
compilation model, with no grammar changes — every new form is an ordinary
call or block binding.

**`feedback(body)` with an ambient `fb` signal.** Faust's `~` operator is
point-free; the voice DSL is applied, so the loop is written as an expression
instead: inside `feedback(...)` the reserved name `fb` is the loop's previous
output (one sample late), and the expression's value is its current output.
A feedback echo reads
`wet = feedback(dry + fb |> delay(0.25) |> gain(0.6))`. The compiler gives
the spec DAG one relaxation: a new `VoiceSignalRef::Feedback(i)` may point at
the current or a later node (validated against the node-list length), closing
a cycle. Lowering collects the tapped nodes, threads their one-sample-delayed
values as extra bus channels, and wraps the whole DAG in the `Rec` combinator
with an identity feedback path — so loops genuinely lower onto `feedback()`
rather than a bespoke evaluator. `fb` placeholders are patched to the loop's
root node index after its body compiles, which nests correctly.

**`fan(input, branch, branch, ...)`** (also pipeable:
`x |> fan(lowpass(500, 0.2), lowpass(3000, 0.2))`) is split-then-merge: the
input feeds every branch — each branch is a stage call or pipe chain that
receives it as its piped-in first argument — and the branch outputs are
summed. Duplication reuses `wire`'s channel fan-out (the split semantics the
lowering already had); the fan-in is a new n-ary `Merge` spec node lowered
onto the `Mrg` combinator.

**Supporting stages.** `delay(x, seconds)` exposes the fixed `delay_line`
(literal seconds, capped at 10 s, capacity allocated at build time) and
`gain(x, amount)` is `Mul` as a pipeable stage, since `*` cannot follow a
pipe target grammatically.

**Modulatable delay time.** When the `delay` time argument is not a number
literal it is compiled as a SIGNAL — `wet = dry |> delay(lfo)` is the
chorus/flanger form — lowering onto the graph layer's fractional delay line
(`fdelay`, linearly interpolated, time input modulatable at audio rate). The
line's capacity stays fixed before the audio thread runs: signal-driven
delays get one second of headroom
(`MODULATED_VOICE_DELAY_MAX_SECONDS`), and requested times outside
\[0, capacity\] clamp at render time. Literal times keep the exact
whole-sample `delay_line`, so existing bodies are byte-for-byte unchanged.

**Pragma bindings.** Two reserved binding names configure the program rather
than defining signals, keeping the grammar untouched (named call arguments do
not survive general evaluation, the same reason `voice` is a block):
`poly = n` sets the program's pooled polyphony (integer literal, 1–64;
`GraphVoiceSpec::with_polyphony`, default 8 per ADR 0009), and `release = s`
floors the release tail so feedback tails ring out past the longest envelope
release (0–30 s; envelope-derived releases still win when longer). Pools are
still built off-thread and swapped at cycle boundaries; the counting-
allocator suite covers the new node kinds and a non-default pool size.
(A third pragma, `steal = oldest|off`, later joined these two when voice
stealing shipped; see ADR 0009's addendum.)

## Addendum: `sample("name")` source stage

The sample-playback gap on the Faust-parity roadmap closed as one more voice
stage over one new graph primitive, giving voice bodies hybrid sample+synth
instruments: `kit = voice { s = sample("bd") ; s * ar(gate, 0.001, 0.2) }`
sends a preloaded drum hit through the same envelope/filter/feedback
vocabulary as any oscillator.

**One node, one-shot semantics.** `SamplePlayerNode`
(`crates/orpheus-dsp/src/graph/sample_player.rs`) is a 2-in (gate, rate)/
1-out player over a sample bank buffer (the bank stores mono, stereo files
downmix at load). A rising gate edge restarts playback from the top; the
gate level is otherwise ignored — one-shot, matching the engine's sample
voices, and composing with ADR 0009's rectangular event-span gate. The rate
is a signal input (1.0 = native pitch), read every frame; fractional
playhead positions interpolate linearly, and playback ends at the buffer end
(loop mode is a follow-up). In a voice body the note gate triggers the stage
implicitly and the optional second argument is the rate signal.

**Buffers resolve at definition time.** `VoiceNodeSpec::Sample` carries the
resolved `PlaybackSample` — an `Arc` handle to the bank's decoded frames —
not a name. The evaluator now holds the sample bank (the session threads its
live bank through `eval_into_bindings_with_samples`; plain `eval_module`
and loaded files use the built-in bank), so `compile_voice` looks the name
up when the `voice { ... }` block evaluates: an unknown sample errors
immediately with the loaded-token list, spec validation still cannot fail
later, and the audio thread never touches the bank. Two accepted
consequences: a voice keeps the buffer it was defined with until it is
redefined, even if the bank reloads underneath it; and files loaded through
the strict loader resolve against the built-in bank only (threading the
session bank through the loader is a follow-up).

**Deterministic voice end still holds.** A one-shot must survive gates
shorter than the buffer, so the stage extends the program's release tail to
at least the sample's duration at native rate, capped by the same 30 s bound
as the `release` pragma (longer samples truncate at the note lifetime's
end). The counting-allocator suite covers a hybrid sample+sine voice: pool
build resolves the handle off-thread, and trigger/render stay
allocation-free.

**Follow-ups shipped: loop mode, pitch tracking, pattern-driven rate.** The
one-shot's deferred items landed as two sibling stages plus a test-proven
wiring of the existing rate input. `sample_loop("name"[, rate])` hard-wraps
the playhead at the buffer end (no crossfade; at rate 1.0 the output is the
buffer tiled bit-exactly, and the interpolator reads toward the buffer head
across the wrap) — the gate keeps one-shot trigger semantics, so a falling
gate never cuts the loop: it sounds until the voice's release tail ends,
exactly like a one-shot outliving its gate, and the stage extends the
release the same way. A separate stage name was chosen over a flag argument
because the grammar has no keyword arguments and the second positional slot
is already the rate signal. `sample_pitched("name"[, reference_hz])` derives
the playback rate from the note: rate = `freq` / reference, computed per
frame inside the player so a note exactly at the reference plays at exactly
1.0 (bit-identical to `sample()`). The default reference is 220 Hz —
`DEFAULT_ANALOG_BASE_FREQUENCY_HZ`, the engine's note-frequency convention
(`freq` = 220 Hz x the event's playback-rate multiplier), so an unshifted
note plays natively and `|> pitch(12)` doubles the rate; a number-literal
argument re-anchors it (`sample_pitched("bd", 7040)` plays natively on c4 =
220 x 2^(60/12)). Both lower onto the existing `SamplePlayerNode` via
`looped`/`pitch_reference_hz` fields on `VoiceNodeSpec::Sample`; the
counting-allocator suite covers looped and pitched pooled voices. Finally,
the rate argument was already a signal, so per-note pattern control needed
no new surface: `sample("bd", p1)` with `hits |> p1(1 2)` sets each note's
rate from the pattern side (now locked in by end-to-end tests). The
looped+pitched combination then shipped as
`sample_loop_pitched("name"[, reference_hz])` — the DSP layer already
composed the `looped`/`pitch_reference_hz` flags, so the stage is pure
surface, a fourth sibling name (same no-keyword-arguments reasoning as
`sample_loop`: the second positional slot is the reference) with
`sample_pitched`'s validation and 220 Hz default. Shipping it also changed
the release mechanism for pitched samples: the static tail covers the buffer
at NATIVE rate, so a note far below the reference (rate << 1, playback much
longer) used to truncate mid-take. The rate depends on the triggering note
and is unknown at spec-build time, so the bank now floors the release tail
per note at trigger time — each program keeps its worst-case
`duration x reference` product (`GraphVoiceSpec::pitched_sample_tail_hz_seconds`)
and the trigger stamps `max(static release, product / freq)` on the note's
existing per-note release field (the allocation-free shape of the
gain/pan/param stamping), capped by the same 30 s bound
(`MAX_VOICE_RELEASE_TAIL_SECONDS`); notes at or above the reference stamp
the static release bit-for-bit. The final follow-up — crossfaded loop
points — then shipped as `sample_loop_xf("name"[, rate])` and
`sample_loop_pitched_xf("name"[, reference_hz])`: an opt-in `loop_crossfade`
flag on `VoiceNodeSpec::Sample` (`sample_player_looped_crossfaded` at the
node layer) blends the loop tail into the buffer head over a short LINEAR
crossfade — 5 ms of source material, capped at 10% of the buffer, computed
from the two read positions on the fly with no fade buffer — so loops that
do not end on a zero crossing stop clicking at the wrap. Linear
(constant-gain) rather than equal-power because the two reads come from the
same correlated material, where equal-gain blending preserves amplitude; the
head's first `fade` samples double as crossfade material, so the
steady-state loop period is `len - fade` (buffers under one whole fade
sample keep the hard wrap). The crossfade is opt-in via separate stage names
(the same no-keyword-arguments reasoning as `sample_loop`) because the hard
wrap's bit-exact tiling is a documented, test-locked property of
`sample_loop` that sample-accurate material relies on.

## Addendum: pattern-side control signals (`p1`..`p4`)

The last deferred engine-integration item — pattern-side controls reaching
INSIDE a voice — shipped as four general-purpose per-note parameters rather
than a re-plumbing of the existing named controls.

**Four ambient parameter signals.** A voice body now sees `p1`..`p4`
alongside `gate` and `freq`: `acid = voice { f = saw(freq) |> svf_lp(p1,
0.7) ; f * ar(gate, 0.001, 0.05) }`. On the pattern side, `p1`..`p4` are
ordinary patternable controls with the same shape as `gain`
(`melody |> p1(300 6000)` sets each note's `p1` per event;
`|> p1(<200 800>)` alternates per cycle). A parameter is dimensionless — a
cutoff in Hertz, a detune amount, a morph position — its meaning is the voice
body's to define, so any finite number validates.

**Generic parameters, not the named controls.** Wiring `cutoff`/`res`/etc.
into voice bodies was rejected: those controls already act on the engine
side, AFTER the voice (sample-voice insert filters and the analog fallback
path), so reusing the names would either double-apply them or silently
change meaning depending on the instrument. The generic names make the
routing explicit and leave every existing control byte-for-byte unchanged.
(`p`/`param` itself was unavailable — it is the plugin-parameter automation
builtin — so the surface is the four indexed controls directly.)

**Per-note sample-and-hold semantics.** The parameter value is resolved when
the event is scheduled (`SampleEvent`/`SampleTrigger` carry a
`[f64; VOICE_PARAM_COUNT]` field) and stamped on the pooled note as plain
`f32` fields at trigger time — the allocation-free shape of the gain/pan
fields from the stealing addendum. Inside the graph the parameters ride the
fixed interface as four trailing signal inputs
(`[gate, freq, gain, pan, p1..p4]`), constant for the note's lifetime. A
steal stamps the NEW note's parameters immediately (like frequency; only
gain/pan ramp through the handover). Smooth per-note ramping and audio-rate
pattern control remain follow-ups.

**Documented default: 0.** A body referencing a parameter the pattern never
sets reads `DEFAULT_VOICE_PARAM_VALUE` (0.0) — chosen over a per-stage
"neutral" value because a parameter has no intrinsic unit; bodies wanting a
baseline write it explicitly (`svf_lp(p1 + 200, 0.7)`). Zero composes
naturally with `+` offsets and multiplies to silence, matching the gate's
convention.

## Addendum: multi-mode SVF and peaking-EQ filter stages

The graph layer's multi-mode filters (TPT SVF and RBJ biquad, ADR 0004
follow-up) reached voice bodies as five stages: `svf_lp`/`svf_hp`/`svf_bp`/
`svf_notch` take `(input, cutoff_hz, q)` — the `lowpass` convention, one
stage name per response rather than a mode argument, since voice-body
identifiers resolve as signals and cannot carry an enum — and
`eq_peak(x, freq_hz, q, gain_db)` exposes the peaking biquad. The shelving
follow-up then shipped: `BiquadMode::LowShelf`/`HighShelf` (RBJ cookbook,
the same gain\_db fourth input channel and clamps as peaking) reach voice
bodies as `eq_low_shelf(x, freq_hz, q, gain_db)` / `eq_high_shelf(...)`,
lowered onto `biquad(sr, LowShelf|HighShelf)` through
`VoiceNodeSpec::EqShelf` (a `ShelfMode` field rather than two variants,
mirroring `Svf`'s mode field). All parameters are signals, and the SVF
recomputes coefficients per sample, so `saw(freq) |> svf_lp(lfo, 0.7)`
sweeps the cutoff at audio rate. Lowering keeps the one-output-per-node bus
shape by pairing the four-output `SvfNode` with a fixed-width channel
selector (`wire_with_inputs`, a `wire` variant whose input width is explicit
so unselected responses are dropped); `EqPeak` lowers directly onto
`biquad(sr, Peaking)`. Number-literal parameters are range-checked at
definition time against the same public clamp bounds the nodes apply at
render time (`FILTER_MIN_FREQUENCY_HZ`, `FILTER_MIN_Q`/`FILTER_MAX_Q`,
`FILTER_MAX_GAIN_DB`); bound signals clamp per sample instead. The
counting-allocator suite covers an LFO-swept SVF plus peaking boost through
a pooled bank.
