# 6. The Orca grid

Everything so far has been *textual* pattern code. Orpheus also ships a second,
completely different sequencing surface: an **Orca grid**, modelled on the
[Orca](https://100r.co/site/orca.html) esoteric livecoding language. This
chapter is a short orientation — for the full design, see
[docs/design/orca-surface.md](../design/orca-surface.md).

## What it is

The Orca surface is a 2D grid of single-character cells. Each frame, the grid is
"ticked": operators (the letters `A`–`Z`) read their neighbours and rewrite
cells, and IO operators emit events. The grid ticks a fixed number of frames per
cycle (16 by default), and frame *N* maps exactly onto the rational time span
`[N/F, (N+1)/F)` — so the grid sits on the very same cycle clock as your
patterns, and the two stay in lockstep.

Where text patterns describe rhythm declaratively, the grid is spatial and
procedural: you place operators in a plane and watch data flow between them.
It excels at generative, self-modifying sequences.

## What it drives

The grid is fundamentally an **IO surface**. Its output operators send events out
of Orpheus over three real transports, on a dedicated `orca-io` thread that never
touches the audio path:

- **MIDI** out (plus optional MIDI clock)
- **OSC** messages
- **raw UDP** datagrams

That makes the grid a natural controller for external synths, hardware, visuals,
or other software — anything that speaks MIDI or OSC.

## Configuring it: the `:orca` commands

The grid lives in the TUI, and you point its transports with the `:orca` command
family. A few of the essentials:

```text
:orca                          # show current targets, MIDI port, clock, listener
:orca udp <host:port | port>   # UDP target (default 127.0.0.1:49161)
:orca osc <host:port | port>   # OSC target (default 127.0.0.1:49162)
:orca midi list                # list MIDI output ports
:orca midi connect <port>      # connect one by exact name
:orca midi clock on|off        # emit MIDI clock (default off)
:orca listen [on | off | <host:port>]   # accept UDP commands (default off)
```

The naming mirrors the session's `:midi` commands: transports are addressed by
name or address, never by device index.

## Where to go next

The grid has a full operator set, velocity/length controls, comments, and a
`$`/UDP command interpreter for driving it from outside. All of that is beyond
this tutorial's scope; the
[Orca surface design doc](../design/orca-surface.md) is the complete reference,
including the operator tables and transport details.

For now, know that it is there when you want a spatial, generative counterpart to
the text patterns — and that both share one clock. The
[final chapter](07-capstone.md) returns to text patterns to build a complete
piece.
