# 7. Putting it together

Time to assemble everything into one piece. We will build a short arrangement —
drums, a bass synth, and a lead — and save it as an `.ode` file you can load and
play. The finished file is
[docs/examples/tutorial_capstone.ode](../examples/tutorial_capstone.ode); we
build it up here one layer at a time.

> `.ode` files are evaluated in **strict** mode and have no comment syntax — each
> line is a binding (long expressions may wrap across lines inside brackets). Type
> the same lines in the REPL to develop them live, then paste them into a file.

## Layer 1: the drums

Three named patterns, combined with `stack`, then widened in stereo. We reuse
the notation and transforms from chapters 2 and 3:

```text
kick  = bd ~ bd ~ |> mask(euclid(5, 8))
snare = ~ sn ~ sn |> when(4, 2, rev)
hats  = hh*8 |> gain(0.6) |> degrade
drums = stack(kick, snare, hats) |> jux(rev)
```

A euclidean kick, a snare that flips every fourth cycle, a degraded hat run —
stacked and split left/right with `jux(rev)`.

## Layer 2: a bass instrument

Define a sawtooth `voice` (chapter 4), then drive it with a note pattern. The
`pitch` control feeds each note's frequency into the voice's `freq`:

```text
bass_synth = voice { osc = saw(freq) ; shaped = osc |> lowpass(800, 0.3) |> drive(1.2) ; shaped * ar(gate, 0.001, 0.2) }
bassline = bass_synth bass_synth bass_synth bass_synth |> pitch(c2 e2 g2 c3) |> gain(0.5)
```

## Layer 3: a lead

A second, brighter voice — a plain sine with a quick envelope. We arpeggiate it
and speed it up every fourth cycle for movement:

```text
lead_synth = voice { sine(freq) * ar(gate, 0.001, 0.08) }
lead = lead_synth ~ lead_synth ~ |> pitch(c4 e4 g4 c5) |> every(4, fast(2)) |> gain(0.4)
```

## The whole piece

Stack the three layers into one `song` binding. `bassline |> slow(2)` stretches
the bass over two cycles so it moves half as often as the drums:

```text
song = stack(
  drums,
  bassline |> slow(2),
  lead
)
```

## Playing it

Save the four blocks above to a file (or use the shipped
`docs/examples/tutorial_capstone.ode`) and launch:

```sh
cargo run --release -- docs/examples/tutorial_capstone.ode
```

Then, at the prompt:

```text
> :tempo 130
> :play
```

You now hear the full arrangement. Because every binding is still live, you can
keep editing — redefine `lead`, add a `reverb` send with `:bus`/`:send`
(chapter 5), or `:render song mix.wav 8` to bounce it to disk.

## Where to go from here

- Sweeten the mix with the mixer and effect buses from
  [chapter 5](05-mixing-and-export.md).
- Reach for a specific builtin in the
  [language reference](../reference/language-reference.md).
- Explore the [Orca grid](06-orca-grid.md) for generative, spatial sequencing.
- Read the other shipped example,
  [analog_showcase.ode](../examples/analog_showcase.ode), for a denser
  synth-driven arrangement.

That is the whole loop of Orpheus: type a pattern, hear it, transform it, layer
it, and shape it into a piece — all without stopping the sound. Happy
live-coding.
