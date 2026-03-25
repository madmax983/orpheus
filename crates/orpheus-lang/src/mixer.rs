//! The `mixer` module manages the audio routing and effects state.
//!
//! This module acts as the bridge between the high-level pattern language
//! and the DSP audio engine's routing graph. It manages tracks, buses,
//! effects (like delay and reverb), and sends.
//!
//! # Concepts
//!
//! - **Tracks:** Endpoints that consume evaluated pattern events (like `"bd"`)
//!   and produce audio. By default, patterns play on the `main` compatibility track.
//! - **Buses:** Auxiliary channels that process audio via effects (like Delay or Reverb).
//! - **Sends:** Connections that route a portion of a track's audio to a bus.

use std::collections::BTreeMap;
use std::fmt::Write;

use orpheus_dsp::{RoutingSnapshot, SampleTrigger, TrackSource};
use orpheus_pattern::Event;
use orpheus_pattern::Rational;

use crate::Value;

/// The configuration state of the audio mixer.
///
/// A `MixerState` instance records user-defined tracks, buses, effects, and sends.
/// It acts as a builder to generate a [`RoutingSnapshot`], which the DSP backend
/// applies transactionally to avoid audio dropouts.
///
/// # Examples
///
/// Creating a new mixer with a custom bus effect and track send:
///
/// ```
/// use orpheus_lang::mixer::MixerState;
/// use orpheus_pattern::Rational;
///
/// let mut mixer = MixerState::default();
///
/// // Create a delay bus
/// mixer.new_bus("fx1").unwrap();
/// mixer.set_bus_delay("fx1", Rational::new(1, 4), 0.5, 0.8).unwrap();
///
/// // Route a track to the bus
/// mixer.new_track("lead").unwrap();
/// mixer.set_send("lead", "fx1", 0.6).unwrap();
///
/// assert!(mixer.has_routing_state());
/// ```
#[derive(Clone, Debug, Default)]
pub struct MixerState {
    compatibility_main_binding: Option<String>,
    tracks: BTreeMap<String, MixerTrack>,
    buses: BTreeMap<String, MixerBus>,
}

#[derive(Clone, Debug)]
struct MixerTrack {
    binding_name: Option<String>,
    level: f32,
    muted: bool,
    sends: BTreeMap<String, f32>,
}

#[derive(Clone, Debug, Default)]
struct MixerBus {
    effect: Option<MixerBusEffect>,
}

#[derive(Clone, Debug)]
enum MixerBusEffect {
    Delay {
        time: Rational,
        feedback: f32,
        wet: f32,
    },
    Reverb {
        size: f32,
        damp: f32,
        wet: f32,
    },
}

impl Default for MixerTrack {
    fn default() -> Self {
        Self {
            binding_name: None,
            level: 1.0,
            muted: false,
            sends: BTreeMap::new(),
        }
    }
}

impl MixerState {
    pub(crate) fn note_sample_binding(&mut self, name: &str) {
        self.compatibility_main_binding = Some(name.to_owned());
    }

    pub(crate) fn has_routing_state(&self) -> bool {
        !self.tracks.is_empty() || !self.buses.is_empty()
    }

    pub(crate) fn has_explicit_bound_tracks(&self) -> bool {
        self.tracks
            .values()
            .any(|track| track.binding_name.as_deref().is_some())
    }

    pub(crate) fn new_track(&mut self, name: &str) -> Result<(), String> {
        if name == "main" {
            return Err("track `main` is reserved for compatibility playback".to_owned());
        }
        if name == "master" {
            return Err("track `master` is reserved".to_owned());
        }
        if self.tracks.contains_key(name) {
            return Err(format!("track `{name}` already exists"));
        }
        if self.buses.contains_key(name) {
            return Err(format!("routing name `{name}` is already used by a bus"));
        }
        self.tracks.insert(name.to_owned(), MixerTrack::default());
        Ok(())
    }

    pub(crate) fn bind_track(
        &mut self,
        track_name: &str,
        binding_name: &str,
        bindings: &BTreeMap<String, Value>,
    ) -> Result<(), String> {
        ensure_sample_binding(binding_name, bindings)?;
        let track = self
            .tracks
            .get_mut(track_name)
            .ok_or_else(|| format!("no track named `{track_name}`"))?;
        track.binding_name = Some(binding_name.to_owned());
        Ok(())
    }

    pub(crate) fn set_track_level(&mut self, track_name: &str, level: f32) -> Result<(), String> {
        if !level.is_finite() || level < 0.0 {
            return Err("track level must be a finite value >= 0".to_owned());
        }
        let track = self
            .tracks
            .get_mut(track_name)
            .ok_or_else(|| format!("no track named `{track_name}`"))?;
        track.level = level;
        Ok(())
    }

    pub(crate) fn set_track_mute(&mut self, track_name: &str, muted: bool) -> Result<(), String> {
        let track = self
            .tracks
            .get_mut(track_name)
            .ok_or_else(|| format!("no track named `{track_name}`"))?;
        track.muted = muted;
        Ok(())
    }

    pub(crate) fn new_bus(&mut self, name: &str) -> Result<(), String> {
        if name == "master" {
            return Err("bus `master` is reserved".to_owned());
        }
        if name == "main" {
            return Err("bus `main` conflicts with the reserved compatibility track".to_owned());
        }
        if self.buses.contains_key(name) {
            return Err(format!("bus `{name}` already exists"));
        }
        if self.tracks.contains_key(name) {
            return Err(format!("routing name `{name}` is already used by a track"));
        }
        self.buses.insert(name.to_owned(), MixerBus::default());
        Ok(())
    }

    pub(crate) fn set_bus_delay(
        &mut self,
        bus_name: &str,
        time: Rational,
        feedback: f32,
        wet: f32,
    ) -> Result<(), String> {
        if time <= Rational::zero() {
            return Err("delay time must be a positive rational like 1/8".to_owned());
        }
        if !feedback.is_finite() || !(0.0..=1.0).contains(&feedback) {
            return Err("delay feedback must be a finite value in [0, 1]".to_owned());
        }
        if !wet.is_finite() || !(0.0..=1.0).contains(&wet) {
            return Err("delay wet must be a finite value in [0, 1]".to_owned());
        }

        let bus = self
            .buses
            .get_mut(bus_name)
            .ok_or_else(|| format!("no bus named `{bus_name}`"))?;
        bus.effect = Some(MixerBusEffect::Delay {
            time,
            feedback,
            wet,
        });
        Ok(())
    }

    pub(crate) fn set_bus_reverb(
        &mut self,
        bus_name: &str,
        size: f32,
        damp: f32,
        wet: f32,
    ) -> Result<(), String> {
        if !size.is_finite() || !(0.0..=1.0).contains(&size) {
            return Err("reverb size must be a finite value in [0, 1]".to_owned());
        }
        if !damp.is_finite() || !(0.0..=1.0).contains(&damp) {
            return Err("reverb damp must be a finite value in [0, 1]".to_owned());
        }
        if !wet.is_finite() || !(0.0..=1.0).contains(&wet) {
            return Err("reverb wet must be a finite value in [0, 1]".to_owned());
        }

        let bus = self
            .buses
            .get_mut(bus_name)
            .ok_or_else(|| format!("no bus named `{bus_name}`"))?;
        bus.effect = Some(MixerBusEffect::Reverb { size, damp, wet });
        Ok(())
    }

    pub(crate) fn clear_bus_effect(&mut self, bus_name: &str) -> Result<(), String> {
        let bus = self
            .buses
            .get_mut(bus_name)
            .ok_or_else(|| format!("no bus named `{bus_name}`"))?;
        bus.effect = None;
        Ok(())
    }

    pub(crate) fn set_send(
        &mut self,
        track_name: &str,
        bus_name: &str,
        level: f32,
    ) -> Result<(), String> {
        if !level.is_finite() || !(0.0..=1.0).contains(&level) {
            return Err("send level must be a finite value in [0, 1]".to_owned());
        }
        if !self.buses.contains_key(bus_name) {
            return Err(format!("no bus named `{bus_name}`"));
        }
        let track = self
            .tracks
            .get_mut(track_name)
            .ok_or_else(|| format!("no track named `{track_name}`"))?;
        track.sends.insert(bus_name.to_owned(), level);
        Ok(())
    }

    pub(crate) fn render_summary(&self) -> String {
        self.track_summary_lines()
            .into_iter()
            .chain(self.bus_summary_lines())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub(crate) fn compile_snapshot(
        &self,
        bindings: &BTreeMap<String, Value>,
    ) -> Result<RoutingSnapshot, String> {
        let mut builder = RoutingSnapshot::builder();

        if !self.has_explicit_bound_tracks() {
            builder = match self.compatibility_main_binding.as_deref() {
                Some(binding_name) => builder
                    .track_with_source("main", compile_track_source(binding_name, bindings)?)
                    .route("main", "master"),
                None => builder.main_track(),
            };
        }

        for bus_name in self.buses.keys() {
            builder = builder.bus(bus_name.as_str());
        }

        for (track_name, track) in &self.tracks {
            let source = match track.binding_name.as_deref() {
                Some(binding_name) => compile_track_source(binding_name, bindings)?,
                None => TrackSource::Unbound,
            };
            builder = builder
                .track_with_source_and_mix(
                    track_name.as_str(),
                    source,
                    track.level,
                    0.0,
                    track.muted,
                )
                .route(track_name.as_str(), "master");
        }

        for (track_name, track) in &self.tracks {
            for (bus_name, level) in &track.sends {
                builder = builder.send(track_name.as_str(), bus_name.as_str(), *level);
            }
        }

        for (bus_name, bus) in &self.buses {
            if let Some(effect) = &bus.effect {
                match effect {
                    MixerBusEffect::Delay {
                        time,
                        feedback,
                        wet,
                    } => {
                        builder = builder.bus_effect_delay(
                            bus_name.as_str(),
                            time.clone(),
                            *feedback,
                            *wet,
                        );
                    }
                    MixerBusEffect::Reverb { size, damp, wet } => {
                        builder = builder.bus_effect_reverb(bus_name.as_str(), *size, *damp, *wet);
                    }
                }
            }
        }

        builder.build().map_err(|error| error.to_string())
    }

    pub(crate) fn track_summary_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();

        if !self.has_explicit_bound_tracks() {
            if let Some(binding_name) = &self.compatibility_main_binding {
                lines.push(format!("main -> {binding_name} (auto)"));
            } else {
                lines.push("main -> <unbound> (auto)".to_owned());
            }
        }

        for (track_name, track) in &self.tracks {
            let binding_name = track.binding_name.as_deref().unwrap_or("<unbound>");
            let mut line = format!("{track_name} -> {binding_name}");
            if track.muted {
                line.push_str(" [muted]");
            }
            if (track.level - 1.0).abs() > f32::EPSILON {
                write!(&mut line, " level {:.2}", track.level)
                    .expect("writing to String should not fail");
            }
            for (bus_name, level) in &track.sends {
                write!(&mut line, " +send {bus_name}@{level:.2}")
                    .expect("writing to String should not fail");
            }
            lines.push(line);
        }

        lines
    }

    pub(crate) fn bus_summary_lines(&self) -> Vec<String> {
        self.buses
            .iter()
            .map(|(bus_name, bus)| {
                let mut line = format!("bus {bus_name} -> master");
                if let Some(effect) = &bus.effect {
                    write!(&mut line, " {}", effect.summary())
                        .expect("writing to String should not fail");
                }
                line
            })
            .collect()
    }
}

impl MixerBusEffect {
    fn summary(&self) -> String {
        match self {
            Self::Delay {
                time,
                feedback,
                wet,
            } => format!(
                "delay({} fb{feedback:.2} wet{wet:.2})",
                format_rational(time)
            ),
            Self::Reverb { size, damp, wet } => {
                format!("reverb(size={size:.2} damp={damp:.2} wet={wet:.2})")
            }
        }
    }
}

fn ensure_sample_binding(
    binding_name: &str,
    bindings: &BTreeMap<String, Value>,
) -> Result<(), String> {
    let value = bindings
        .get(binding_name)
        .ok_or_else(|| format!("no binding named `{binding_name}`"))?;
    if value.as_sample_pattern().is_some() {
        Ok(())
    } else {
        Err(format!(
            "binding `{binding_name}` is a {} and cannot be assigned to a track",
            value.kind_name()
        ))
    }
}

fn compile_track_source(
    binding_name: &str,
    bindings: &BTreeMap<String, Value>,
) -> Result<TrackSource, String> {
    ensure_sample_binding(binding_name, bindings)?;
    let pattern = bindings
        .get(binding_name)
        .and_then(Value::as_sample_pattern)
        .ok_or_else(|| format!("no binding named `{binding_name}`"))?;
    let events = pattern.query_unit().map_err(|error| {
        format!("failed to query unit span for track binding `{binding_name}`: {error}")
    })?;
    Ok(TrackSource::SamplePattern(
        events
            .into_iter()
            .map(sample_event_to_trigger_event)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    ))
}

fn sample_event_to_trigger_event(event: Event<crate::SampleEvent>) -> Event<SampleTrigger> {
    Event {
        whole: event.whole,
        part: event.part,
        value: {
            let mut trigger = SampleTrigger::named(event.value.sample())
                .with_gain(event.value.gain())
                .with_pan(event.value.pan())
                .with_rate(event.value.rate())
                .with_resonance(event.value.resonance())
                .with_drive(event.value.drive())
                .with_pulse_width(event.value.pulse_width())
                .with_slice(event.value.slice_start(), event.value.slice_end());
            if let Some(cutoff_hz) = event.value.hpf_cutoff_hz() {
                trigger = trigger.with_hpf_cutoff_hz(cutoff_hz);
            }
            if let Some(cutoff_hz) = event.value.lpf_cutoff_hz() {
                trigger = trigger.with_lpf_cutoff_hz(cutoff_hz);
            }
            trigger
        },
    }
}

fn format_rational(value: &Rational) -> String {
    format!("{}/{}", value.numerator(), value.denominator())
}
