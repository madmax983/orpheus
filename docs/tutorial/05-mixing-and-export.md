# 5. Mixing and export

You have patterns and instruments playing together. This chapter is about
*balance* — routing your bindings through a mixer — and about getting the result
out of Orpheus as audio, stems, or MIDI.

All of the tools here are `:` commands, so they run in the session rather than in
a pattern expression.

## The mixer model

Orpheus has a small mixer with three moving parts:

- **Tracks** carry the audio from one binding. You give a track a name and bind a
  pattern to it, then set its level and mute state.
- **Buses** are shared effect destinations — a reverb or delay every track can
  send to.
- **Sends** route a slice of a track's signal to a bus.

### Tracks

```text
> drums = bd sn cp sn
> :track new beat
> :track bind beat drums
> :track level beat 0.8
> :track mute beat off
```

`:track new <name>` creates a track; `:track bind <track> <binding>` attaches a
pattern to it; `:track level <track> <n>` sets its gain; `:track mute <track>
on|off` silences or restores it.

### Buses and effects

Create a bus and host an effect on it. Buses support `delay` and `reverb`:

```text
> :bus new space
> :bus fx space reverb
```

`:bus fx <bus> reverb` (or `delay`) attaches the effect, accepting optional
parameters after the effect name; `:bus fx <bus> none` clears it.

### Sends

Route a track to a bus at a send level in `0..1`:

```text
> :send beat space 0.3
```

Thirty percent of the `beat` track now feeds the `space` reverb bus.

### Seeing the mixer

`:mixer` prints the current tracks, buses, and sends in a table:

```text
> :mixer
```

## Rendering to audio

`:render <binding> <path> <cycles>` bounces a binding to an audio file — `.wav`
or `.flac` — for a given number of cycles. This works even on a headless machine
where live playback is disabled:

```text
> :render drums drums.wav 4
```

Four cycles of `drums` written to `drums.wav`.

## Exporting

`:export` is the general exporter. Point it at a binding, a path, and a cycle
count, and it chooses the format from the extension (MIDI, SVG, and more):

```text
> :export melody melody.mid 8
```

To bounce each track to its own file for mixing elsewhere, use the stems form:

```text
> :export stems song stems/ 8
```

This writes one file per track into the `stems/` directory. (The companion
`:import stems <dir>` loads a directory of stem WAVs back in as sample patterns
and routed tracks.)

## MIDI out

Beyond `.mid` file export, Orpheus can play a binding out of a live MIDI port.
The `:midi` family manages inputs and outputs:

```text
> :midi list
> :midi connect <port>
> :midi send melody 1
```

`:midi list` shows available output ports, `:midi connect <port>` opens one, and
`:midi send <binding> [channel]` streams a binding to it (channel defaults
sensibly). There is a matching `:midi in ...` family for taking MIDI *in* —
including `:midi in map-note <note> <binding>` to trigger bindings from a
keyboard. (On Linux, MIDI needs the same `libasound2-dev` package as audio.)

With a mixed, exportable arrangement you have the whole studio. The
[next chapter](06-orca-grid.md) is a short detour into a completely different way
to sequence — the Orca grid — before the [capstone](07-capstone.md) ties
everything together.
