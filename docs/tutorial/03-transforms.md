# 3. Transforms

Notation gives you rhythms; **transforms** reshape them. A transform is a
function that takes a pattern and returns a new one. You apply them with the
pipe operator `|>`, which reads left to right: "take this pattern, then do
this, then do this."

```text
d = bd sn cp sn |> fast(2) |> rev
```

Read it as: the sequence `bd sn cp sn`, played twice as fast, then reversed.
Because most transforms take the pattern as their last argument, they compose
cleanly under `|>` — `fast(2)` is a transform waiting for a pattern, and the
pipe feeds it one.

## Time: `fast`, `slow`, `rev`

```text
d = bd sn |> fast(2)
```

`fast(n)` packs `n` cycles of the pattern into one (so `bd sn` becomes
`bd sn bd sn`); `slow(n)` stretches one cycle across `n`. `rev` plays the cycle
backwards:

```text
d = bd sn cp hh |> rev
```

## Periodic transforms: `every`, `when`, `whenmod`, `within`

These apply a transform only *sometimes*, which is what turns a loop into an
arrangement.

`every(n, f)` applies `f` on every `n`-th cycle:

```text
d = bd sn cp sn |> every(4, rev)
```

Three cycles play straight, the fourth reversed, forever.

`when(n, offset, f)` applies `f` on cycles where `cycle % n == offset`;
`whenmod(n, offset, f)` is the same idea with the modulo phrased for longer
spans:

```text
d = bd sn |> whenmod(4, 2, fast(2))
```

`within(start, end, f)` applies `f` only to the fraction of each cycle between
`start` and `end` (both in `0..1`):

```text
d = bd sn cp hh |> within(0, 0.5, rev)
```

Only the first half of the cycle is reversed each time.

## Randomness: `degrade`, `sometimes`, `often`, `rarely`

`degrade` randomly drops events (the function form of the `?` modifier);
`degrade_by(p)` sets the drop probability. `sometimes(f)` applies `f` to a
random ~50% of events, with `often` (~75%), `rarely` (~25%), `almost_always`,
and `almost_never` as presets:

```text
d = hh*8 |> degrade
```

```text
d = hh*8 |> sometimes(fast(2))
```

All randomness is deterministic — seeded by event position — so a pattern is
reproducible across runs but feels alive.

## Displacement: `off`, `rot`, `chunk`

`off(time, f)` layers a copy of the pattern, shifted later by `time` and passed
through `f` — instant echoes and call-and-response:

```text
d = bd sn |> off(0.25, gain(0.5))
```

`rot(n)` rotates the *values* of a pattern by `n` steps while the rhythm stays
put. `chunk(n, f)` splits the cycle into `n` parts and applies `f` to a
different part each cycle, marching across:

```text
d = bd sn cp hh |> chunk(4, rev)
```

## Stereo: `jux`

`jux(f)` plays the original pattern in the left channel and an `f`-transformed
copy in the right — a stereo widener that is the signature Tidal move:

```text
d = bd sn cp hh |> jux(rev)
```

Straight on the left, reversed on the right.

## Continuous signals: `segment`, `range`

Some patterns are *continuous* — `rand` is a smooth stream of random values, not
discrete events. `segment(n, sig)` samples a continuous signal into `n` discrete
steps per cycle, and `range(lo, hi)` rescales a `0..1` signal into a useful
range:

```text
n = rand |> segment(8) |> range(200, 2000)
```

Eight random values per cycle between 200 and 2000 — perfect for driving a
filter cutoff or a note parameter.

## Euclid as a function

Besides the inline `bd(3,8)` form, `euclid(pulses, steps)` is a standalone
boolean pattern you can mask with:

```text
d = bd*8 |> mask(euclid(5, 8))
```

`mask` keeps only the events that line up with the euclidean `true` slots.

## Effect controls

Controls attach audio parameters to the events in a pattern. They pipe like any
transform:

| Control | Effect |
| --- | --- |
| `gain(x)` | Loudness multiplier (`1.0` is unity) |
| `pan(x)` | Stereo position, `0` left ... `1` right |
| `lpf(hz)` | Low-pass filter cutoff |
| `hpf(hz)` | High-pass filter cutoff |
| `reverb(x)` | Reverb send amount |

```text
d = bd sn cp sn |> gain(0.8) |> lpf(1200) |> reverb(0.2)
```

Because control arguments are themselves patterns, you can make them move over
time. A per-event cutoff sweep:

```text
d = hh*8 |> lpf(200 800 2000 8000)
```

## Layering

`stack(...)` combines named patterns into one — the function form of the
top-level comma, and the usual way to assemble a track:

<a id="layering"></a>

```text
kick  = bd ~ bd ~
snare = ~ sn ~ sn
hats  = hh*8 |> gain(0.6)
drums = stack(kick, snare, hats)
```

## Building up a groove

Watch a groove grow one pipe at a time. Start plain:

```text
d = bd sn cp sn
```

Add a euclidean kick underneath and a degraded hat run on top:

```text
d = bd(3,8), ~ sn ~ sn, hh*8?
```

Widen it in stereo and reverse it every fourth cycle:

```text
d = bd(3,8), ~ sn ~ sn, hh*8? |> jux(rev) |> every(4, fast(2))
```

That is the whole live-coding workflow: type, hear, pipe, repeat. When you want
these events to play a *synth* instead of samples, you build an instrument —
that is the [next chapter](04-instruments.md).
