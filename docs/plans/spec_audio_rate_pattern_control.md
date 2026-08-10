# Spec: Audio-Rate Pattern Control

## User Story
As a Sound Designer, I want to modulate voice parameters at audio rates using patterns, so that I can create complex, evolving timbres like FM synthesis directly from the sequencer.

## The "So What?"
What business problem does this solve? It bridges the gap between composition and sound design by allowing composers to express high-frequency modulation without leaving the language, turning the sequencer into a modular synthesizer. This unlocks expressive capabilities critical for professional electronic music producers.

## Metric Definition
Success = Audio-rate modulation executes with 0 runtime allocations on the audio thread and maintains stable CPU load.

## Gap Analysis
TidalCycles handles control-rate parameters well, leaving audio-rate modulation to external synths like SuperDirt. Pure Data and Max/MSP excel at audio-rate graphs but lack intuitive code-based sequencing. Orpheus combining pattern syntax with audio-rate modulation offers a unique hybrid.

## Acceptance Criteria
- Must evaluate parameter patterns at the audio sample rate without frame drops.
- Must not panic when evaluating complex nested patterns at high frequencies.
- Must integrate smoothly with the existing voice DSL.

## Out of Scope
- Cross-voice modulation.
