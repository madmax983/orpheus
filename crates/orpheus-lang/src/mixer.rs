#![allow(clippy::format_push_string)]
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

use comfy_table::{Cell, Table, presets::UTF8_BORDERS_ONLY};
use crossterm::style::Stylize;

use ratatui::style::{Color as TuiColor, Modifier as TuiModifier, Style as TuiStyle};
use ratatui::text::{Line, Span};

use orpheus_dsp::{RoutingSnapshot, SampleTrigger, TrackSource};
use orpheus_pattern::Event;
use orpheus_pattern::Rational;

use crate::Value;
use crate::export::sample_trigger_from_event;

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
/// ```ignore
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

    pub(crate) fn render_tui_summary(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        let header_style = TuiStyle::default().fg(TuiColor::DarkGray);
        let track_style = TuiStyle::default().fg(TuiColor::Cyan);
        let binding_style = TuiStyle::default().fg(TuiColor::Yellow);
        let level_style = TuiStyle::default().fg(TuiColor::Green);

        self.render_tui_tracks_table(
            &mut lines,
            header_style,
            track_style,
            binding_style,
            level_style,
        );

        if !self.buses.is_empty() {
            self.render_tui_buses_table(&mut lines, header_style, track_style, level_style);
        }

        lines
    }

    fn render_tui_tracks_table(
        &self,
        lines: &mut Vec<Line<'static>>,
        header_style: TuiStyle,
        track_style: TuiStyle,
        binding_style: TuiStyle,
        level_style: TuiStyle,
    ) {
        lines.push(Line::from(vec![Span::styled(
            "Mixer Tracks:",
            track_style.add_modifier(TuiModifier::BOLD),
        )]));

        let mut track_rows = Vec::new();
        track_rows.push(vec![
            Span::styled("Track", header_style),
            Span::styled("Binding", header_style),
            Span::styled("Level", header_style),
            Span::styled("Muted", header_style),
            Span::styled("Sends", header_style),
        ]);

        if self.has_explicit_bound_tracks() {
            for (track_name, track) in &self.tracks {
                let binding = track.binding_name.as_deref().unwrap_or("<unbound>");
                let sends = track
                    .sends
                    .iter()
                    .map(|(bus, level)| format!("{bus} @ {level:.2}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let muted_color = if track.muted {
                    TuiColor::Red
                } else {
                    TuiColor::DarkGray
                };

                track_rows.push(vec![
                    Span::styled(track_name.to_owned(), track_style),
                    Span::styled(binding.to_owned(), binding_style),
                    Span::styled(format!("{:.2}", track.level), level_style),
                    Span::styled(track.muted.to_string(), TuiStyle::default().fg(muted_color)),
                    Span::styled(sends, header_style),
                ]);
            }
        } else {
            let binding = self
                .compatibility_main_binding
                .as_deref()
                .unwrap_or("<unbound>");
            track_rows.push(vec![
                Span::styled("main (auto)".to_owned(), track_style),
                Span::styled(binding.to_owned(), binding_style),
                Span::styled("1.00".to_owned(), level_style),
                Span::styled("false".to_owned(), header_style),
                Span::styled(String::new(), header_style),
            ]);
        }

        let mut col_widths = [0; 5];
        for row in &track_rows {
            for (i, col) in row.iter().enumerate() {
                col_widths[i] = col_widths[i].max(col.content.len());
            }
        }

        for row in track_rows {
            let mut spans = Vec::new();
            for (i, col) in row.into_iter().enumerate() {
                let padding = col_widths[i].saturating_sub(col.content.len());
                let padded_content = format!("{}{}", col.content, " ".repeat(padding));
                spans.push(Span::styled(padded_content, col.style));
                if i < 4 {
                    spans.push(Span::raw(" │ "));
                }
            }
            lines.push(Line::from(spans));
        }
    }

    fn render_tui_buses_table(
        &self,
        lines: &mut Vec<Line<'static>>,
        header_style: TuiStyle,
        track_style: TuiStyle,
        level_style: TuiStyle,
    ) {
        lines.push(Line::from(vec![Span::raw("")]));
        lines.push(Line::from(vec![Span::styled(
            "Mixer Buses:",
            track_style.add_modifier(TuiModifier::BOLD),
        )]));

        let mut bus_rows = Vec::new();
        bus_rows.push(vec![
            Span::styled("Bus", header_style),
            Span::styled("Effect", header_style),
        ]);

        for (bus_name, bus) in &self.buses {
            let effect = bus
                .effect
                .as_ref()
                .map_or_else(|| "none".to_owned(), MixerBusEffect::summary);
            bus_rows.push(vec![
                Span::styled(bus_name.to_owned(), track_style),
                Span::styled(effect, level_style),
            ]);
        }

        let mut bus_col_widths = [0; 2];
        for row in &bus_rows {
            for (i, col) in row.iter().enumerate() {
                bus_col_widths[i] = bus_col_widths[i].max(col.content.len());
            }
        }

        for row in bus_rows {
            let mut spans = Vec::new();
            for (i, col) in row.into_iter().enumerate() {
                let padding = bus_col_widths[i].saturating_sub(col.content.len());
                let padded_content = format!("{}{}", col.content, " ".repeat(padding));
                spans.push(Span::styled(padded_content, col.style));
                if i < 1 {
                    spans.push(Span::raw(" │ "));
                }
            }
            lines.push(Line::from(spans));
        }
    }

    pub(crate) fn render_summary(&self) -> String {
        let mut output = String::new();

        let mut track_table = Table::new();
        track_table.load_preset(UTF8_BORDERS_ONLY);
        track_table.set_header(vec![
            Cell::new("Track").fg(comfy_table::Color::DarkGrey),
            Cell::new("Binding").fg(comfy_table::Color::DarkGrey),
            Cell::new("Level").fg(comfy_table::Color::DarkGrey),
            Cell::new("Muted").fg(comfy_table::Color::DarkGrey),
            Cell::new("Sends").fg(comfy_table::Color::DarkGrey),
        ]);

        if self.has_explicit_bound_tracks() {
            for (track_name, track) in &self.tracks {
                let binding = track.binding_name.as_deref().unwrap_or("<unbound>");
                let sends = track
                    .sends
                    .iter()
                    .map(|(bus, level)| format!("{bus} @ {level:.2}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                let muted_color = if track.muted {
                    comfy_table::Color::Red
                } else {
                    comfy_table::Color::DarkGrey
                };
                track_table.add_row(vec![
                    Cell::new(track_name).fg(comfy_table::Color::Cyan),
                    Cell::new(binding).fg(comfy_table::Color::Yellow),
                    Cell::new(format!("{:.2}", track.level)).fg(comfy_table::Color::Green),
                    Cell::new(track.muted.to_string()).fg(muted_color),
                    Cell::new(sends).fg(comfy_table::Color::DarkGrey),
                ]);
            }
        } else {
            let binding = self
                .compatibility_main_binding
                .as_deref()
                .unwrap_or("<unbound>");
            track_table.add_row(vec![
                Cell::new("main (auto)").fg(comfy_table::Color::Cyan),
                Cell::new(binding).fg(comfy_table::Color::Yellow),
                Cell::new("1.00").fg(comfy_table::Color::Green),
                Cell::new("false").fg(comfy_table::Color::DarkGrey),
                Cell::new(String::new()).fg(comfy_table::Color::DarkGrey),
            ]);
        }

        let _ = std::fmt::Write::write_fmt(
            &mut output,
            format_args!("{}\n", "Mixer Tracks:".cyan().bold()),
        );
        output.push_str(&track_table.to_string());

        if !self.buses.is_empty() {
            let mut bus_table = Table::new();
            bus_table.load_preset(UTF8_BORDERS_ONLY);
            bus_table.set_header(vec![
                Cell::new("Bus").fg(comfy_table::Color::DarkGrey),
                Cell::new("Effect").fg(comfy_table::Color::DarkGrey),
            ]);

            for (bus_name, bus) in &self.buses {
                let effect = bus
                    .effect
                    .as_ref()
                    .map_or_else(|| "none".to_owned(), MixerBusEffect::summary);
                bus_table.add_row(vec![
                    Cell::new(bus_name).fg(comfy_table::Color::Cyan),
                    Cell::new(effect).fg(comfy_table::Color::Green),
                ]);
            }

            let _ = std::fmt::Write::write_fmt(
                &mut output,
                format_args!("\n\n{}\n", "Mixer Buses:".cyan().bold()),
            );
            output.push_str(&bus_table.to_string());
        }

        output
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
                            *time,
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
            .iter()
            .map(sample_event_to_trigger_event)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    ))
}

fn sample_event_to_trigger_event(event: &Event<crate::SampleEvent>) -> Event<SampleTrigger> {
    Event {
        whole: event.whole,
        part: event.part,
        value: sample_trigger_from_event(&event.value),
    }
}

fn format_rational(value: &Rational) -> String {
    format!("{}/{}", value.numerator(), value.denominator())
}
