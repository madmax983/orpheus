# Effect Routing

> Note (2026-03-23): The architectural framing in this older note has been
> superseded by the routing snapshot mixer design in
> `docs/plans/2026-03-23-routing-and-mixer-architecture-design.md` and
> `docs/adr/0003-routing-snapshot-mixer-architecture.md`.
>
> Shared effects now sit on top of the explicit track/bus routing spine rather
> than arriving as a standalone bus feature. The first concrete follow-on slice
> is specified in `docs/plans/2026-03-23-shared-delay-bus-design.md`, and the
> next shared-space operator is specified in
> `docs/plans/2026-03-23-shared-reverb-bus-design.md`.
>
> In particular, the mixer control plane now uses explicit `:send` and `:bus fx`
> commands. Pattern-language `send(...)` routing remains deferred.

## 👤 User Story
"As a Live Coder, I want the ability to route audio through global send effects (like a shared reverb or delay bus) in addition to per-pattern insert effects, so that I can create cohesive, glued-together mixes without consuming excessive CPU power by instantiating duplicate effects on every single pattern layer."

## ❓ The "So What?" (Business Problem)
Currently, if a performer wants three different drum layers and a synth lead to share the same virtual space, they must apply an individual reverb effect to each pattern directly. This is not only computationally expensive (running four independent reverb algorithms instead of one), but it also fragments the mix. In traditional DAWs, engineers solve this using "Send/Return" aux tracks. By introducing a global effects bus and send routing to Orpheus, we allow artists to build more complex, professional-sounding arrangements while significantly reducing DSP overhead. Complexity is a cost; utility is revenue. Optimizing the effects architecture gives users back valuable CPU headroom, empowering them to add more layers and intricacy to their live sets.

## 🎯 Metric Definition
- **Success** = Users can route multiple patterns to a single, shared effect bus via the language (e.g., `send("reverb", 0.5)`). CPU utilization for 10 patterns routed to a shared reverb is at least 60% lower than instantiating 10 individual per-pattern reverbs, with zero audio dropouts during routing changes on the real-time thread.

## 🔍 Gap Analysis
- **Current State (Orpheus):** All effects are treated as per-pattern "inserts" within the DSP graph. There is no concept of a shared bus or aux send.
- **Competitors (TidalCycles/SuperDirt, Sonic Pi):** SuperDirt relies on global orbits and effect buses to handle shared reverb and delay efficiently. Sonic Pi uses `with_fx` blocks which can enclose multiple synths to share an effect instance.
- **The Gap:** Orpheus needs a hybrid routing model that supports both immediate, per-pattern insert effects (for sound design) and global, named effect buses (for mixing and optimization).

## ✅ Acceptance Criteria
- Must introduce a way to instantiate global, named effect buses in the REPL or via a configuration script (e.g., `:bus new reverb(room=0.8)`).
- Must introduce a `send("bus_name", level)` function in the pattern language to route a portion of a pattern's audio signal to the named bus.
- Must ensure that the global buses process audio and mix it back into the master output synchronously with the main pattern outputs.
- Must support sequencing the `level` parameter via patterns (e.g., `send("reverb", slow(4, seq(0.1, 0.9)))`).
- Must handle routing changes seamlessly on the real-time audio thread without allocation, locking, or audio clicking.

## 🚫 Out of Scope
- Infinite feedback loop prevention if buses are routed into themselves (Phase 1 will strictly prohibit bus-to-bus routing).
- Multi-channel surround sound routing (e.g., 5.1 or 7.1). Phase 1 is strictly for stereo master buses.
- Sidechain compression routing across buses. This will require a more complex audio graph traversal logic in Phase 2.
