# Effect Routing Architecture Spec

## 👤 User Story
"As a Producer, I want to route audio through global send effects (like a shared reverb) and apply insert effects per-layer, so that I can create cohesive mixes and preserve CPU overhead instead of spawning a new reverb instance for every single drum hit."

## ❓ So What? (Business Problem)
Currently, transformations in Orpheus apply logic per-event. While this is powerful for granular sound design (like changing a filter cutoff per note), applying heavy DSP effects like large reverbs or long delays on a per-event or even per-pattern basis is a massive CPU drain and leads to muddy, disconnected mixes. Professional music production relies on "send" and "return" buses to place disparate sounds in the same acoustic space. Without a bus routing architecture, Orpheus is constrained to simple loops and will crash or glitch under the CPU load of complex tracks. Complexity is a cost, but failing to support standard mixing paradigms makes the software useless for serious composition. Utility is revenue.

## 📊 Metric Definition
- **Success** = A full song with 10+ active layers routed to a single shared reverb bus runs without audio dropouts (< 50% CPU load on the audio thread).
- **Success** = The syntax allows seamlessly sending a pattern to a named bus with a specific send amount.

## ✅ Acceptance Criteria
- Must introduce a syntax for defining global effect buses at the session level (e.g., `bus("reverb", reverb(0.8, 1.2))`).
- Must allow patterns to route a portion of their signal to these named buses via a `send` transformation (e.g., `drums |> send("reverb", 0.5)`).
- Must support insert effects at the layer/pattern level (applied to the summed audio output of the pattern, rather than instantiated per-event).
- DSP engine must correctly sum all incoming send signals and process them through the bus effect exactly once per audio block.

## 🚫 Out of Scope
- Feedback routing (routing a bus back into itself or into a previous bus in the chain).
- Sidechain compression routing (using the audio envelope of one bus to duck the volume of another). This will be addressed in a future Phase 2 spec.
- Multi-channel surround sound routing (e.g., Atmos/5.1). Orpheus remains strictly stereo for now.
