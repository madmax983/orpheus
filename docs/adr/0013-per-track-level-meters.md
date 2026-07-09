# ADR 0013: Per-Track Lock-Free Level Meters

- Status: Accepted
- Date: 2026-07-09

## Context

The mixer surface (`orpheus-lang`) shows each track's configured gain in a
"Level" column, but that value is a static piece of routing state — it never
moves with the audio. Users have no live feedback about which tracks are
actually producing signal, how hot they are, or whether a track is clipping.

The engine already publishes a lock-free `TransportSnapshot` (ADR 0003 /
`engine.rs`): the audio thread writes plain atomics with `Ordering::Relaxed`
and the UI reads them without locks via `EngineHandle::transport_snapshot()`.
We want per-track signal metering that follows exactly the same discipline, so
the **audio callback stays allocation-free and lock-free** (the hard constraint
in `CLAUDE.md`).

## Decision

Add a per-track peak meter that the audio thread updates in place and the UI
reads through a lock-free `LevelSnapshot`, mirroring `TransportSnapshot`.

### Key design choices

**Peak-hold with per-frame exponential decay.**
Each rendered frame, for every track we take the post-fader magnitude
`max(|left|, |right|)` and fold it into the running peak with
`peak = (peak * decay).max(magnitude)`. A fresh loud sample wins instantly
(the `max`), and when the signal stops the peak falls smoothly toward zero.
This gives a natural VU/peak-hold feel, is a single multiply-and-max per track,
and needs no history buffer — trivially real-time safe. The decay factor is
derived once at engine construction from the sample rate for a ~200 ms
half-life (`decay = 0.5^(1 / (half_life_secs * sample_rate))`), so tempo and
render-block size never affect the ballistics.

**Post-fader, mute-aware measurement.**
The magnitude is sampled after the track gain (`track.level()`) is applied, so
the meter reflects what actually reaches the master bus. A muted track feeds a
magnitude of `0.0`, so its meter decays away rather than showing phantom signal.

**Fixed-capacity atomic array, indexed by `TrackId`.**
Meter storage is `[AtomicU32; MAX_METERED_TRACKS]` (`MAX_METERED_TRACKS = 64`),
one `f32`-as-bits slot per track id — the same index space as the engine's
`track_mix_buffer`. The array is preallocated in `SharedMeters`; the audio
thread only ever stores into it. There is **no `Vec` growth and no allocation**
on the callback path. Track ids at or above the capacity are ignored (bounded
write), which is safe because realistic routings use a handful of tracks.

**Audio-thread-local accumulator, published per block.**
The running peaks live as plain `f32` in `EngineCore` (`meter_peaks`), updated
per frame with no atomics. At the end of each `render_into_interleaved` call the
renderer publishes them into `SharedMeters` with `f32::to_bits` +
`AtomicU32::store(Relaxed)` — the same cadence and ordering as the transport
publish. Stopping the transport and shrinking the routing zero the retired
slots so meters do not freeze at a stale value.

**No seqlock for the meter snapshot.**
Unlike `TransportSnapshot`, meters do not need the even/odd epoch seqlock. Each
slot is a single `u32`, so a reader never observes a torn `f32`, and
cross-track consistency within one frame is irrelevant for a moving meter — a
one-block skew between two bars is imperceptible. `Relaxed` loads/stores are
therefore sufficient and cheapest.

### Memory layout

```text
SharedMeters { peaks: [AtomicU32; 64] }   // Arc-shared, UI <-> audio thread
EngineCore.meter_peaks: [f32; 64]         // audio-thread-local accumulator
LevelSnapshot { peaks: [f32; 64] }        // Copy, returned to the UI
```

### Audio-thread safety argument

- The callback only performs `f32` arithmetic and `AtomicU32::store(Relaxed)`
  into a preallocated array — no locks, no syscalls, no allocation.
- `SharedMeters` is `Arc`-shared; no `Mutex`/`RwLock` is involved.
- The UI reads via `AtomicU32::load(Relaxed)` and never blocks the audio thread.

## Consequences

- The TUI mixer table gains a live per-track "Meter" column, Theme-colored
  (accent → warning → error as the level approaches clip), plus a pure
  `meter_bar(level, width)` / `meter_color(level)` helper pair reusable at any
  pane width.
- `LevelSnapshot` / `EngineHandle::meter_snapshot()` join `TransportSnapshot`
  as the second lock-free UI-readable engine view; `ReplSession::meter_view()`
  and `SharedState::meter_view()` surface it exactly like `transport_view()`.
- Existing audio output is unchanged: the meter path only reads the mix values
  the engine already computes and writes atomics beside them.
- The 64-track ceiling is a soft limit for metering only (not for routing); a
  larger fixed capacity is a one-line change if ever needed.
