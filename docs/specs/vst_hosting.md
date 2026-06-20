# 🔭 Vantage: Spec for VST3/AU Plugin Hosting

## 👤 User Story
As a Live-Coding Musician, I want to host external VST3/AU instrument plugins within Orpheus, so that I can sequence high-quality third-party synthesizers and samplers using Orpheus's pattern language without routing to external applications.

## 🎯 The "So What?"
**What business problem does this solve?**
Orpheus currently relies on its internal sound generation. While this is great for self-contained composition, it limits the sonic palette. Many musicians have heavily invested in third-party plugins. Supporting these plugins turns Orpheus into a viable, professional sequencing brain, dramatically increasing the target market and retention.

## 📏 Metric Definition
- **Success:** Loading a plugin takes < 500ms.
- **Success:** Audio renders reliably without dropouts.
- **Success:** Note events hit the plugin with sample-accurate timing matching the Orpheus cycle.

## 🕵️ Gap Analysis
- **Current State:** Orpheus only supports internal sample playback.
- **Market Standard:** Other live coding environments and DAWs natively host third-party plugins.
- **The Gap:** We need a host implementation that translates Orpheus patterns into note and parameter automation lanes that the plugin can consume.

## ✅ Acceptance Criteria
- Must be able to discover installed VST3/AU plugins from standard OS paths.
- Must support sending note events to the plugin.
- Must support mapping Orpheus pattern controls to plugin parameters.
- Must execute the plugin's audio generation block and output the result.
- Must operate headlessly (no plugin GUI during standard live coding operation).

## 🚫 Out of Scope
- Plugin GUI rendering (Phase 2).
- VST2 support (deprecated format).
- Audio effect plugins (FX). This spec covers instrument plugins (generators) only for Phase 1.