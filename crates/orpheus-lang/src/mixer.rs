use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use orpheus_dsp::{RoutingSnapshot, SampleTrigger, TrackSource};
use orpheus_pattern::Event;

use crate::Value;

#[derive(Clone, Debug, Default)]
pub struct MixerState {
    compatibility_main_binding: Option<String>,
    tracks: BTreeMap<String, MixerTrack>,
    buses: BTreeSet<String>,
}

#[derive(Clone, Debug)]
struct MixerTrack {
    binding_name: Option<String>,
    level: f32,
    muted: bool,
    sends: BTreeMap<String, f32>,
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
        if self.buses.contains(name) {
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
        if self.buses.contains(name) {
            return Err(format!("bus `{name}` already exists"));
        }
        if self.tracks.contains_key(name) {
            return Err(format!("routing name `{name}` is already used by a track"));
        }
        self.buses.insert(name.to_owned());
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
        if !self.buses.contains(bus_name) {
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

        for bus_name in &self.buses {
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
            .map(|bus_name| format!("bus {bus_name} -> master"))
            .collect()
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
