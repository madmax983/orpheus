# 4. Instruments

So far every event has played a sample. Now you build your own **synth voices**
with the `voice { }` block and drive them from patterns. This is Orpheus's Faust
side: a voice is a small signal graph compiled to a real-time instrument.

## A first voice

A voice body is a sequence of `let`-style bindings separated by `;`, ending in
one result expression — the mono signal the voice outputs:

```text
pluck = voice { osc = saw(freq) ; env = adsr(gate, 0.001, 0.02, 0.5, 0.05) ; osc * env }
```

Read it top to bottom: build a sawtooth oscillator at the note's frequency,
build an ADSR envelope driven by the note's gate, and multiply them so the
oscillator is shaped by the envelope. Binding it registers `pluck` as a new
pattern token.

### Ambient signals

Inside a voice body, a few names are always in scope, supplied by the note that
triggers the voice:

- `freq` — the note's frequency in Hz (derived from the reference frequency and
  the event's pitch).
- `gate` — `1` while the note is held, `0` when it releases. Feed it to
  envelopes.
- `fb` — the feedback signal, available only inside a `feedback(...)` loop.
- `p1`, `p2`, `p3`, `p4` — per-note parameters the pattern can set (below).

## Driving a voice with notes

A voice token plays like any other token. Sequence it, and set the pitches with
the `pitch` control:

```text
bass = pluck pluck pluck |> pitch(c2 e2 g2)
```

`c2 e2 g2` are named pitch literals (lowercase, with an octave; `fs4` and `bf3`
are sharp/flat spellings). Each successive note drives the voice's `freq`, so
you hear an ascending arpeggio through your pluck.

You can also rest between hits (`pluck ~ pluck ~`) or fire a single sustained
note (`pluck ~ ~ ~`), exactly like samples.

## The vocabulary

### Oscillators

`sine(freq)`, `saw(freq)`, `tri(freq)`, `pulse(freq)` (optionally
`pulse(freq, width)`), and `noise()`. Frequency is usually the ambient `freq`,
but any signal works — `sine(2)` is a 2 Hz LFO.

### Envelopes

`adsr(gate, attack, decay, sustain, release)` and the two-stage
`ar(gate, attack, release)`. Their time arguments must be literal numbers:

```text
stab = voice { saw(freq) * ar(gate, 0.001, 0.15) }
```

If a voice has no envelope, Orpheus still gives it a short release tail so notes
do not click off.

### Filters

`lowpass(x, cutoff, q)` is the workhorse. The state-variable family
`svf_lp`, `svf_hp`, `svf_bp`, `svf_notch(x, cutoff, q)` recomputes coefficients
every sample, so their cutoff and Q accept *moving* signals for audio-rate
sweeps. The EQ stages `eq_peak(x, freq, q, gain_db)`, `eq_low_shelf`, and
`eq_high_shelf` shape tone:

```text
acid = voice { body = saw(freq) + tri(freq) * 0.5 ; shaped = body |> lowpass(1200, 0.3) |> drive(1.5) ; shaped * ar(gate, 0.001, 0.08) }
```

Note the `|>` pipe works *inside* a voice body too, threading the signal through
each stage, and `+`/`*` mix and scale signals.

### Shaping

`drive(x, amount)` overdrives, `gain(x, amount)` scales, and `delay(x, time)`
adds a delay line. A literal `time` gives a fixed delay; a *signal* time gives a
fractional, modulatable delay (the basis of a flanger).

### Feedback and parallel banks

`feedback(body)` closes a loop: inside it, the ambient `fb` is the loop's own
delayed output. This is how you build an echo that lives inside the instrument:

```text
echo = voice { release = 0.5 ; dry = sine(freq) * ar(gate, 0.001, 0.01) ; wet = feedback(dry + fb |> delay(0.05) |> gain(0.5)) ; dry + wet }
```

`fan(x, branch1, branch2, ...)` splits a signal into parallel branches and sums
them — a filter bank in one line:

```text
bank = voice { osc = saw(freq) ; fan(osc, lowpass(500, 0.2), lowpass(3000, 0.2)) * ar(gate, 0.001, 0.05) }
```

### Sample stages

A voice can play a sample-bank buffer, so you can filter and envelope samples
just like synth oscillators. `sample("bd")` plays it once; `sample("bd", rate)`
sets a playback rate; `sample_loop`, `sample_loop_xf` (crossfaded), and
`sample_pitched` are the looping and pitch-tracking variants:

```text
hybrid = voice { s = sample("bd") ; body = s + sine(freq) * 0.2 ; body |> lowpass(2000, 0.1) |> gain(ar(gate, 0.001, 0.1)) }
```

## Per-note parameters: `p1`..`p4`

The `p1`..`p4` ambient signals let the *pattern* reach into the *voice*. Read
one in the body, then set it from the pattern side with the matching control:

```text
acid = voice { f = saw(freq) |> svf_lp(p1, 0.7) ; f * ar(gate, 0.001, 0.05) }
line = acid acid |> p1(300 6000)
```

The first note filters at 300 Hz, the second at 6000 Hz — the same instrument,
two different timbres, chosen by the pattern. Unset parameters read `0`. If a
`p1` control has sub-note structure (`p1(100 200 400 800)` under one note), the
values become breakpoints and the parameter *ramps* across the held note.

## Pragmas

Special bindings at the top of a body configure the voice rather than its
signal:

| Pragma | Meaning |
| --- | --- |
| `poly = n` | Pooled polyphony, `1..=64` (default 8) |
| `release = seconds` | Floor for the release tail, up to 30 s |
| `steal = oldest \| off` | What to do when the voice pool is exhausted |
| `param_ramp = seconds` | Glide `p1`..`p4` on retrigger/steal, `0..=1` s |

```text
lead = voice { poly = 4 ; release = 0.2 ; sine(freq) * ar(gate, 0.001, 0.05) }
```

## A minimal voice to start from

You do not need a complicated patch to get going. The smallest useful synth is a
single oscillator shaped by a short envelope — copy this and change one thing at
a time:

```text
beep = voice { sine(freq) * ar(gate, 0.001, 0.08) }
melody = beep beep beep beep |> pitch(c4 e4 g4 c5)
```

Swap `sine` for `saw`, add a `|> lowpass(...)` stage, or drop in a `drive` — each
edit hot-swaps at the next cycle boundary, so you hear exactly what each stage
does.

With instruments in hand, the last step is balancing several of them together
and getting the result out of Orpheus — that is
[mixing and export](05-mixing-and-export.md).
