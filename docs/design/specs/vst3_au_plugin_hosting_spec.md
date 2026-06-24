# 🔭 Vantage: Spec for VST3/AU Plugin Hosting

## 👤 User Story
As a Live-Coder, I want to host external VST3 and AU instrument plugins within Orpheus, so that I can sequence industry-standard software synthesizers and samplers using Orpheus's pattern language.

## 💼 Business Value
Orpheus currently relies on its internal DSP engine. By supporting VST3 and AU plugins, we instantly unlock the entire ecosystem of professional virtual instruments. This bridges the gap between pure algorithmic experimentation and professional-grade music production, massively increasing Orpheus's utility.

## 🎯 Success Metrics
* **Latency:** Plugin processing adds < 5ms of overhead to the audio callback.
* **Compatibility:** Successfully loads and plays back audio from major standard plugins across Windows (VST3), macOS (VST3/AU), and Linux (VST3).
* **Stability:** 0 crashes under a load of 8 concurrent plugin instances.

## 🕳️ Gap Analysis
Currently, users would have to route MIDI out of Orpheus into a DAW to get professional sounds, breaking the "single environment" experience. Integrating a headless plugin host natively keeps the user entirely within Orpheus.

## ✅ Acceptance Criteria
* The host must be able to load VST3 plugins on Windows, macOS, and Linux, and AU plugins on macOS.
* Must support sending MIDI note-on and note-off events with velocity to the plugin.
* Must support parameter lanes (automation) to manipulate plugin parameters.
* Must run headlessly (without opening the plugin's GUI).
* Must preallocate processing buffers to ensure real-time audio thread safety.

## 🚫 Out of Scope
* Graphical User Interfaces (GUIs) for the plugins.
* VST2 plugin format support.
* Audio Effect plugins.
