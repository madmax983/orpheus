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

use comfy_table::{Cell, CellAlignment, Table, presets::UTF8_BORDERS_ONLY};
use crossterm::style::Stylize;

use ratatui::style::{Modifier as TuiModifier, Style as TuiStyle};
use ratatui::text::{Line, Span};

use crate::tui::style::{Theme, meter_bar, meter_color};

use orpheus_dsp::{GeneratorCycleSpec, GeneratorId, RoutingSnapshot, SampleTrigger, TrackSource};
use orpheus_pattern::Event;
use orpheus_pattern::Rational;
use orpheus_pattern::TimeSpan;

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
    /// Bindings backed by an engine-side generator slot (ADR 0009). Tracks
    /// bound to these names compile to [`TrackSource::Generator`] instead of
    /// a static sample pattern, so per-cycle buffers pushed over the ring
    /// keep flowing regardless of snapshot recompiles.
    generator_bindings: BTreeMap<String, GeneratorId>,
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

    /// Marks `name` as backed by the engine generator slot `generator_id`
    /// (ADR 0009). Snapshot compiles resolve the binding to
    /// [`TrackSource::Generator`] from here on.
    pub(crate) fn note_generator_binding(&mut self, name: &str, generator_id: GeneratorId) {
        self.generator_bindings
            .insert(name.to_owned(), generator_id);
    }

    pub(crate) fn has_routing_state(&self) -> bool {
        !self.tracks.is_empty() || !self.buses.is_empty()
    }

    pub(crate) fn has_explicit_bound_tracks(&self) -> bool {
        self.tracks
            .values()
            .any(|track| track.binding_name.as_deref().is_some())
    }

    pub(crate) fn contains_routing_name(&self, name: &str) -> bool {
        self.tracks.contains_key(name) || self.buses.contains_key(name)
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

    pub(crate) fn render_tui_summary(&self, levels: &[f32]) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        let header_style = TuiStyle::default().fg(Theme::MUTED);
        let track_style = TuiStyle::default().fg(Theme::ACCENT);
        let binding_style = TuiStyle::default().fg(Theme::WARNING);
        let level_style = TuiStyle::default().fg(Theme::SUCCESS);

        self.render_tui_tracks_table(
            &mut lines,
            header_style,
            track_style,
            binding_style,
            level_style,
            levels,
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
        levels: &[f32],
    ) {
        /// Fixed glyph width of the live per-track meter bar (ADR 0013). The
        /// meter is the last column, so it stays readable even in narrow panes.
        const METER_BAR_WIDTH: usize = 8;

        lines.push(Line::from(vec![Span::styled(
            "Mixer Tracks:",
            track_style.add_modifier(TuiModifier::BOLD),
        )]));

        // The meter for each data row corresponds to the routing `TrackId`,
        // which is assigned by track order — the same order rows are emitted
        // here (see `MixerState::compile_snapshot`).
        let meter_cell = |row_index: usize| {
            let level = levels.get(row_index).copied().unwrap_or(0.0);
            Span::styled(
                meter_bar(level, METER_BAR_WIDTH),
                TuiStyle::default().fg(meter_color(level)),
            )
        };

        let mut track_rows = Vec::new();
        track_rows.push(vec![
            Span::styled("Track", header_style),
            Span::styled("Binding", header_style),
            Span::styled("Level", header_style),
            Span::styled("Muted", header_style),
            Span::styled("Sends", header_style),
            Span::styled("Meter", header_style),
        ]);

        if self.has_explicit_bound_tracks() {
            for (row_index, (track_name, track)) in self.tracks.iter().enumerate() {
                let binding = track.binding_name.as_deref().unwrap_or("<unbound>");
                // ⚡ Bolt: Eliminate intermediate Vec and String allocations on the hot path.
                // Replaced `.map(|...| format!(...)).collect::<Vec<_>>().join(", ")`
                // with direct buffered writing into a pre-allocated String.
                let mut sends = String::with_capacity(track.sends.len() * 16);
                for (i, (bus, level)) in track.sends.iter().enumerate() {
                    if i > 0 {
                        sends.push_str(", ");
                    }
                    let _ =
                        std::fmt::Write::write_fmt(&mut sends, format_args!("{bus} @ {level:.2}"));
                }
                let muted_color = if track.muted {
                    Theme::ERROR
                } else {
                    Theme::MUTED
                };

                track_rows.push(vec![
                    Span::styled(track_name.to_owned(), track_style),
                    Span::styled(binding.to_owned(), binding_style),
                    Span::styled(format!("{:.2}", track.level), level_style),
                    Span::styled(track.muted.to_string(), TuiStyle::default().fg(muted_color)),
                    Span::styled(sends, header_style),
                    meter_cell(row_index),
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
                meter_cell(0),
            ]);
        }

        let mut col_widths = [0; 6];
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
                if i < 5 {
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

        let _ = std::fmt::Write::write_fmt(
            &mut output,
            format_args!("{}\n", "Mixer Tracks:".cyan().bold()),
        );
        output.push_str(&self.render_summary_tracks_table());

        if !self.buses.is_empty() {
            let _ = std::fmt::Write::write_fmt(
                &mut output,
                format_args!("\n{}\n", "Mixer Buses:".cyan().bold()),
            );
            output.push_str(&self.render_summary_buses_table());
        }

        output
    }

    fn render_summary_tracks_table(&self) -> String {
        let mut track_table = Table::new();
        track_table.load_preset(UTF8_BORDERS_ONLY);
        track_table.set_header(vec![
            Cell::new("Track")
                .fg(comfy_table::Color::White)
                .add_attribute(comfy_table::Attribute::Bold),
            Cell::new("Binding")
                .fg(comfy_table::Color::White)
                .add_attribute(comfy_table::Attribute::Bold),
            Cell::new("Level")
                .fg(comfy_table::Color::White)
                .add_attribute(comfy_table::Attribute::Bold),
            Cell::new("Muted")
                .fg(comfy_table::Color::White)
                .add_attribute(comfy_table::Attribute::Bold),
            Cell::new("Sends")
                .fg(comfy_table::Color::White)
                .add_attribute(comfy_table::Attribute::Bold),
        ]);

        if self.has_explicit_bound_tracks() {
            for (track_name, track) in &self.tracks {
                let binding = track.binding_name.as_deref().unwrap_or("<unbound>");
                // ⚡ Bolt: Eliminate intermediate Vec and String allocations on the hot path.
                // Replaced `.map(|...| format!(...)).collect::<Vec<_>>().join("\n")`
                // with direct buffered writing into a pre-allocated String.
                let mut sends = String::with_capacity(track.sends.len() * 16);
                for (i, (bus, level)) in track.sends.iter().enumerate() {
                    if i > 0 {
                        sends.push('\n');
                    }
                    let _ =
                        std::fmt::Write::write_fmt(&mut sends, format_args!("{bus} @ {level:.2}"));
                }
                let muted_color = if track.muted {
                    comfy_table::Color::Red
                } else {
                    comfy_table::Color::DarkGrey
                };
                track_table.add_row(vec![
                    Cell::new(track_name).fg(comfy_table::Color::Cyan),
                    Cell::new(binding).fg(comfy_table::Color::Yellow),
                    Cell::new(format!("{:.2}", track.level))
                        .fg(comfy_table::Color::Green)
                        .set_alignment(CellAlignment::Right),
                    Cell::new(track.muted.to_string())
                        .fg(muted_color)
                        .set_alignment(CellAlignment::Right),
                    Cell::new(sends)
                        .fg(comfy_table::Color::DarkGrey)
                        .set_alignment(CellAlignment::Right),
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
                Cell::new("1.00")
                    .fg(comfy_table::Color::Green)
                    .set_alignment(CellAlignment::Right),
                Cell::new("false")
                    .fg(comfy_table::Color::DarkGrey)
                    .set_alignment(CellAlignment::Right),
                Cell::new(String::new())
                    .fg(comfy_table::Color::DarkGrey)
                    .set_alignment(CellAlignment::Right),
            ]);
        }

        track_table.to_string()
    }

    fn render_summary_buses_table(&self) -> String {
        let mut bus_table = Table::new();
        bus_table.load_preset(UTF8_BORDERS_ONLY);
        bus_table.set_header(vec![
            Cell::new("Bus")
                .fg(comfy_table::Color::White)
                .add_attribute(comfy_table::Attribute::Bold),
            Cell::new("Effect")
                .fg(comfy_table::Color::White)
                .add_attribute(comfy_table::Attribute::Bold),
        ]);

        for (bus_name, bus) in &self.buses {
            let effect = bus
                .effect
                .as_ref()
                .map_or_else(|| "none".to_owned(), MixerBusEffect::summary);
            bus_table.add_row(vec![
                Cell::new(bus_name).fg(comfy_table::Color::Cyan),
                Cell::new(effect)
                    .fg(comfy_table::Color::Green)
                    .set_alignment(CellAlignment::Right),
            ]);
        }

        bus_table.to_string()
    }

    pub(crate) fn compile_snapshot(
        &self,
        bindings: &BTreeMap<String, Value>,
    ) -> Result<RoutingSnapshot, String> {
        let mut builder = RoutingSnapshot::builder();

        if !self.has_explicit_bound_tracks() {
            builder = match self.compatibility_main_binding.as_deref() {
                Some(binding_name) => builder
                    .track_with_source(
                        "main",
                        compile_track_source(binding_name, bindings, &self.generator_bindings)?,
                    )
                    .route("main", "master"),
                None => builder.main_track(),
            };
        }

        for bus_name in self.buses.keys() {
            builder = builder.bus(bus_name.as_str());
        }

        for (track_name, track) in &self.tracks {
            let source = match track.binding_name.as_deref() {
                Some(binding_name) => {
                    compile_track_source(binding_name, bindings, &self.generator_bindings)?
                }
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
                        builder =
                            builder.bus_effect_delay(bus_name.as_str(), *time, *feedback, *wet);
                    }
                    MixerBusEffect::Reverb { size, damp, wet } => {
                        builder = builder.bus_effect_reverb(bus_name.as_str(), *size, *damp, *wet);
                    }
                }
            }
        }

        builder.build().map_err(|error| error.to_string())
    }

    /// Compiles a routing snapshot for an **offline master/stem render** that
    /// advances multi-cycle arrangements cycle by cycle.
    ///
    /// [`Self::compile_snapshot`] captures every plain sample-pattern track as a
    /// single-cycle [`TrackSource::SamplePattern`] (its unit-cycle query). The
    /// live engine re-loops that one cycle every bar, which is correct for a
    /// looping pattern but wrong for a finite arrangement such as
    /// `seq_sections(...)`: an offline render of N cycles would replay cycle 0 N
    /// times (issue #1446). This variant instead materializes each such track
    /// across the full `cycles`-cycle timeline, splits it into one cycle-local
    /// event buffer per cycle, and binds the track to a synthetic
    /// [`TrackSource::Generator`] fed by those buffers — the same per-cycle seam
    /// the offline renderer already uses for real generator sources (ADR 0009).
    /// The returned [`GeneratorCycleSpec`] vector carries the synthetic buffers;
    /// pass it (merged with any real generator specs) to
    /// [`orpheus_dsp::render_routing_snapshot_to_master_wav`].
    ///
    /// Tracks already backed by a real engine generator slot keep their existing
    /// [`GeneratorId`] (their recorded buffers are supplied separately by the
    /// session), and plugin tracks are unchanged. Synthetic ids are allocated
    /// above every real generator id so the two never collide.
    ///
    /// # Errors
    ///
    /// Returns a message when a track binding is missing or is not a
    /// sample/plugin pattern, when querying an arrangement fails, or when the
    /// routing fails to build.
    pub(crate) fn compile_offline_snapshot(
        &self,
        bindings: &BTreeMap<String, Value>,
        cycles: u64,
    ) -> Result<(RoutingSnapshot, Vec<GeneratorCycleSpec>), String> {
        let mut builder = RoutingSnapshot::builder();
        let mut specs: Vec<GeneratorCycleSpec> = Vec::new();
        // Synthetic generator ids for materialized sample-pattern tracks start
        // above any real generator slot so they never collide with generators
        // whose recorded buffers the session supplies separately.
        let mut next_generator_id = self
            .generator_bindings
            .values()
            .map(|id| id.get())
            .max()
            .map_or(0, |max| max.saturating_add(1));

        if !self.has_explicit_bound_tracks() {
            builder = match self.compatibility_main_binding.as_deref() {
                Some(binding_name) => builder
                    .track_with_source(
                        "main",
                        self.compile_offline_track_source(
                            binding_name,
                            bindings,
                            cycles,
                            &mut next_generator_id,
                            &mut specs,
                        )?,
                    )
                    .route("main", "master"),
                None => builder.main_track(),
            };
        }

        for bus_name in self.buses.keys() {
            builder = builder.bus(bus_name.as_str());
        }

        for (track_name, track) in &self.tracks {
            let source = match track.binding_name.as_deref() {
                Some(binding_name) => self.compile_offline_track_source(
                    binding_name,
                    bindings,
                    cycles,
                    &mut next_generator_id,
                    &mut specs,
                )?,
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
                        builder =
                            builder.bus_effect_delay(bus_name.as_str(), *time, *feedback, *wet);
                    }
                    MixerBusEffect::Reverb { size, damp, wet } => {
                        builder = builder.bus_effect_reverb(bus_name.as_str(), *size, *damp, *wet);
                    }
                }
            }
        }

        let snapshot = builder.build().map_err(|error| error.to_string())?;
        Ok((snapshot, specs))
    }

    /// Resolves a single track binding to a [`TrackSource`] for an offline
    /// render, materializing plain sample-pattern arrangements as synthetic
    /// per-cycle generators. See [`Self::compile_offline_snapshot`].
    fn compile_offline_track_source(
        &self,
        binding_name: &str,
        bindings: &BTreeMap<String, Value>,
        cycles: u64,
        next_generator_id: &mut u32,
        specs: &mut Vec<GeneratorCycleSpec>,
    ) -> Result<TrackSource, String> {
        // Real generator-backed bindings already advance per cycle: keep their
        // slot id; the session supplies the recorded buffers out-of-band.
        if let Some(generator_id) = self.generator_bindings.get(binding_name) {
            return Ok(TrackSource::Generator(*generator_id));
        }
        ensure_sample_binding(binding_name, bindings)?;
        let value = bindings
            .get(binding_name)
            .ok_or_else(|| format!("no binding named `{binding_name}`"))?;
        if let Some(pattern) = value.as_sample_pattern() {
            let generator_id = GeneratorId::new(*next_generator_id);
            *next_generator_id = next_generator_id.saturating_add(1);
            let materialized = materialize_pattern_cycles(pattern, cycles, binding_name)?;
            specs.push(GeneratorCycleSpec {
                generator_id,
                cycles: materialized,
            });
            return Ok(TrackSource::Generator(generator_id));
        }
        let plugin = value.as_plugin_pattern().ok_or_else(|| {
            format!(
                "binding `{binding_name}` is a {} and cannot be assigned to a track",
                value.kind_name()
            )
        })?;
        Ok(TrackSource::Plugin(plugin.track_source().clone()))
    }
}

/// Materializes `pattern` across `[0, cycles)` and splits the queried events
/// into one cycle-local trigger buffer per cycle index. This is the offline
/// analogue of a per-cycle generator delivery: it lets a finite multi-cycle
/// arrangement (e.g. `seq_sections`) advance section by section during a master
/// render instead of looping cycle 0. Each event's timing is shifted back into
/// its own cycle's `[0, 1)` window so the offline renderer schedules it at the
/// correct within-cycle offset.
fn materialize_pattern_cycles(
    pattern: &crate::value::SamplePatternValue,
    cycles: u64,
    binding_name: &str,
) -> Result<Vec<Box<[Event<SampleTrigger>]>>, String> {
    let end = i64::try_from(cycles).map_err(|_| format!("cycle count {cycles} is too large"))?;
    let span = TimeSpan::new(Rational::from_integer(0), Rational::from_integer(end))
        .map_err(|error| format!("invalid render span for `{binding_name}`: {error}"))?;
    let events = pattern.try_query(&span).map_err(|error| {
        format!("failed to query `{binding_name}` across {cycles} cycle(s): {error}")
    })?;

    let mut per_cycle: Vec<Vec<Event<SampleTrigger>>> = (0..cycles).map(|_| Vec::new()).collect();
    for event in &events {
        let start = event.part.start();
        // The cycle an event belongs to is the floor of its part start; times
        // here are non-negative, so truncating division is the floor.
        let cycle = start.numerator() / start.denominator();
        let Ok(index) = usize::try_from(cycle) else {
            continue;
        };
        if index >= per_cycle.len() {
            continue;
        }
        let shift = |bound: &Rational| -> Result<Rational, String> {
            let numerator = i64::try_from(bound.numerator() - cycle * bound.denominator())
                .map_err(|_| format!("cycle shift overflow for `{binding_name}`"))?;
            let denominator = i64::try_from(bound.denominator())
                .map_err(|_| format!("cycle shift overflow for `{binding_name}`"))?;
            Rational::new(numerator, denominator)
                .map_err(|error| format!("invalid shifted time for `{binding_name}`: {error}"))
        };
        let part = TimeSpan::new(shift(event.part.start())?, shift(event.part.end())?)
            .map_err(|error| format!("invalid shifted part for `{binding_name}`: {error}"))?;
        let whole =
            match event.whole.as_ref() {
                Some(w) => Some(TimeSpan::new(shift(w.start())?, shift(w.end())?).map_err(
                    |error| format!("invalid shifted whole for `{binding_name}`: {error}"),
                )?),
                None => None,
            };
        per_cycle[index].push(Event {
            whole,
            part,
            value: sample_trigger_from_event(&event.value),
        });
    }

    Ok(per_cycle.into_iter().map(Vec::into_boxed_slice).collect())
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
    if value.as_sample_pattern().is_some() || value.as_plugin_pattern().is_some() {
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
    generator_bindings: &BTreeMap<String, GeneratorId>,
) -> Result<TrackSource, String> {
    // Generator-backed bindings (ADR 0009) take precedence over the display
    // binding of the same name: the audible events arrive per cycle over the
    // command ring, not from the statically compiled pattern.
    if let Some(generator_id) = generator_bindings.get(binding_name) {
        return Ok(TrackSource::Generator(*generator_id));
    }
    ensure_sample_binding(binding_name, bindings)?;
    let value = bindings
        .get(binding_name)
        .ok_or_else(|| format!("no binding named `{binding_name}`"))?;
    if let Some(pattern) = value.as_sample_pattern() {
        let events = pattern.query_unit().map_err(|error| {
            format!("failed to query unit span for track binding `{binding_name}`: {error}")
        })?;
        return Ok(TrackSource::SamplePattern(
            events
                .iter()
                .map(sample_event_to_trigger_event)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ));
    }
    let plugin = value.as_plugin_pattern().ok_or_else(|| {
        format!(
            "binding `{binding_name}` is a {} and cannot be assigned to a track",
            value.kind_name()
        )
    })?;
    Ok(TrackSource::Plugin(plugin.track_source().clone()))
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

#[cfg(test)]
mod offline_snapshot_tests {
    use super::*;
    use crate::ReplMode;
    use orpheus_dsp::TrackSource;

    /// A finite multi-cycle arrangement must advance cycle by cycle in an
    /// offline render: `compile_offline_snapshot` materializes it as a synthetic
    /// generator whose per-cycle buffers differ across sections, rather than the
    /// single looping cycle-0 buffer `compile_snapshot` produces (issue #1446).
    #[test]
    fn compile_offline_snapshot_materializes_distinct_cycles() {
        // `<bd sn cp>` alternates a different token every cycle: bd, sn, cp.
        let bindings = crate::eval_module("song = <bd sn cp>", ReplMode::Strict).unwrap();
        let mut mixer = MixerState::default();
        mixer.note_sample_binding("song");

        let (snapshot, specs) = mixer.compile_offline_snapshot(&bindings, 3).unwrap();

        // The plain sample-pattern track is now a synthetic generator source.
        let track = snapshot.track("main").expect("main track present");
        assert!(
            matches!(track.source(), TrackSource::Generator(_)),
            "arrangement track should compile to a per-cycle generator"
        );

        assert_eq!(specs.len(), 1, "exactly one synthetic generator spec");
        let cycles = &specs[0].cycles;
        assert_eq!(cycles.len(), 3, "one buffer per rendered cycle");

        // The three cycles must not be identical — that is the whole point: the
        // arrangement advances (bd -> sn -> cp) instead of looping cycle 0.
        assert_ne!(cycles[0], cycles[1], "cycle 0 and cycle 1 must differ");
        assert_ne!(cycles[1], cycles[2], "cycle 1 and cycle 2 must differ");
        assert_ne!(cycles[0], cycles[2], "cycle 0 and cycle 2 must differ");
        assert!(
            cycles.iter().all(|buffer| !buffer.is_empty()),
            "each cycle should carry an event"
        );
    }

    /// Contrast guard: the *live* single-cycle `compile_snapshot` captures only
    /// cycle 0 as a static `SamplePattern`, which is exactly what looped. This
    /// pins the difference between the two compile paths.
    #[test]
    fn compile_snapshot_captures_only_the_first_cycle() {
        let bindings = crate::eval_module("song = <bd sn cp>", ReplMode::Strict).unwrap();
        let mut mixer = MixerState::default();
        mixer.note_sample_binding("song");

        let snapshot = mixer.compile_snapshot(&bindings).unwrap();
        let track = snapshot.track("main").expect("main track present");
        assert!(
            matches!(track.source(), TrackSource::SamplePattern(_)),
            "live snapshot keeps a static single-cycle sample pattern"
        );
    }
}
