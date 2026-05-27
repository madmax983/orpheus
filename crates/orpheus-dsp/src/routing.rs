//! Phase-1 routing domain for Orpheus DSP.
//!
//! The routing model is intentionally narrow in this slice:
//! - explicit tracks and buses
//! - sample-pattern track sources only
//! - dry buses that route to master by default
//! - optional hosted bus-local effect specs
//! - no insert chains
//! - no bus-to-bus edges

use std::fmt;

use orpheus_pattern::Event;
use orpheus_pattern::Rational;
use thiserror::Error;

use crate::{PluginTrackSource, SampleTrigger};

/// Stable identifier for a track in a routing snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackId(u32);

impl TrackId {
    /// Creates a new `TrackId`.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Extracts the raw numeric identifier used to index into the snapshot's track array.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for TrackId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "track-{}", self.0)
    }
}

/// Stable identifier for a bus in a routing snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BusId(u32);

impl BusId {
    /// Creates a new `BusId`.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Extracts the raw numeric identifier used to index into the snapshot's bus array.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for BusId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bus-{}", self.0)
    }
}

/// The source attached to a track.
#[derive(Clone, Debug, PartialEq)]
pub enum TrackSource {
    /// The track is present but currently unbound.
    Unbound,
    /// A fully resolved unit-cycle sample pattern.
    SamplePattern(Box<[Event<SampleTrigger>]>),
    /// A fully resolved headless plugin instrument track.
    Plugin(PluginTrackSource),
}

impl TrackSource {
    /// Returns `true` if the track is present but has no signal source bound to it.
    #[must_use]
    pub const fn is_unbound(&self) -> bool {
        matches!(self, Self::Unbound)
    }
}

/// A dry send from a track into a bus.
#[derive(Clone, Debug, PartialEq)]
pub struct SendRoute {
    bus_id: BusId,
    level: f32,
}

impl SendRoute {
    #[must_use]
    const fn new(bus_id: BusId, level: f32) -> Self {
        Self { bus_id, level }
    }

    #[must_use]
    pub(crate) const fn bus_id(&self) -> BusId {
        self.bus_id
    }

    #[must_use]
    #[allow(dead_code)]
    pub(crate) const fn level(&self) -> f32 {
        self.level
    }
}

/// Validated shared bus effect configuration carried by a routing snapshot.
#[derive(Clone, Debug, PartialEq)]
pub enum BusEffectSpec {
    /// A delay effect specification.
    Delay(DelaySpec),
    /// A reverb effect specification.
    Reverb(ReverbSpec),
}

impl BusEffectSpec {
    /// Returns a human-readable string identifier for the effect kind (e.g., "delay" or "reverb").
    #[must_use]
    pub const fn kind_name(&self) -> &'static str {
        match self {
            Self::Delay(_) => "delay",
            Self::Reverb(_) => "reverb",
        }
    }
}

/// Tempo-locked shared delay configuration for a bus host.
#[derive(Clone, Debug, PartialEq)]
pub struct DelaySpec {
    time: Rational,
    feedback: f32,
    wet: f32,
}

impl DelaySpec {
    /// Creates a new `DelaySpec`.
    #[must_use]
    pub const fn new(time: Rational, feedback: f32, wet: f32) -> Self {
        Self {
            time,
            feedback,
            wet,
        }
    }

    /// The delay time expressed as a fractional ratio of a musical cycle.
    #[must_use]
    pub const fn time(&self) -> &Rational {
        &self.time
    }

    /// The proportion of the delayed signal that is fed back into the delay line.
    #[must_use]
    pub const fn feedback(&self) -> f32 {
        self.feedback
    }

    /// The volume level of the processed delay signal relative to the dry input.
    #[must_use]
    pub const fn wet(&self) -> f32 {
        self.wet
    }
}

/// Shared diffuse reverb configuration for a bus host.
#[derive(Clone, Debug, PartialEq)]
pub struct ReverbSpec {
    size: f32,
    damp: f32,
    wet: f32,
}

impl ReverbSpec {
    /// Creates a new `ReverbSpec`.
    #[must_use]
    pub const fn new(size: f32, damp: f32, wet: f32) -> Self {
        Self { size, damp, wet }
    }

    /// The simulated physical dimension of the reverberating space.
    #[must_use]
    pub const fn size(&self) -> f32 {
        self.size
    }

    /// The rate at which high frequencies are absorbed by the simulated room walls.
    #[must_use]
    pub const fn damp(&self) -> f32 {
        self.damp
    }

    /// The volume level of the processed reverberation signal relative to the dry input.
    #[must_use]
    pub const fn wet(&self) -> f32 {
        self.wet
    }
}

/// A track node in the routing snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackState {
    id: TrackId,
    name: Box<str>,
    source: TrackSource,
    level: f32,
    pan: f32,
    muted: bool,
    routes_to_master: bool,
    sends: Box<[SendRoute]>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TrackMix {
    level: f32,
    pan: f32,
    muted: bool,
}

impl TrackState {
    #[must_use]
    fn new(
        id: TrackId,
        name: Box<str>,
        source: TrackSource,
        mix: TrackMix,
        routes_to_master: bool,
        sends: Vec<SendRoute>,
    ) -> Self {
        Self {
            id,
            name,
            source,
            level: mix.level,
            pan: mix.pan,
            muted: mix.muted,
            routes_to_master,
            sends: sends.into_boxed_slice(),
        }
    }

    #[must_use]
    pub const fn id(&self) -> TrackId {
        self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    #[must_use]
    pub const fn source(&self) -> &TrackSource {
        &self.source
    }

    #[must_use]
    pub const fn level(&self) -> f32 {
        self.level
    }

    #[must_use]
    pub const fn pan(&self) -> f32 {
        self.pan
    }

    #[must_use]
    pub const fn muted(&self) -> bool {
        self.muted
    }

    #[must_use]
    pub const fn routes_to_master(&self) -> bool {
        self.routes_to_master
    }

    #[must_use]
    fn is_phase1_silent(&self) -> bool {
        self.muted || self.source.is_unbound() || (!self.routes_to_master && self.sends.is_empty())
    }

    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn sends(&self) -> &[SendRoute] {
        &self.sends
    }
}

/// A bus node in the routing snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct BusState {
    id: BusId,
    name: Box<str>,
    routes_to_master: bool,
    effect: Option<BusEffectSpec>,
}

impl BusState {
    #[must_use]
    const fn new(id: BusId, name: Box<str>) -> Self {
        Self {
            id,
            name,
            routes_to_master: true,
            effect: None,
        }
    }

    #[must_use]
    pub const fn id(&self) -> BusId {
        self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    #[must_use]
    pub const fn routes_to_master(&self) -> bool {
        self.routes_to_master
    }

    #[must_use]
    pub const fn effect(&self) -> Option<&BusEffectSpec> {
        self.effect.as_ref()
    }
}

/// Read-only track view exposed by a routing snapshot.
#[derive(Clone, Copy, Debug)]
pub struct TrackView<'a> {
    state: &'a TrackState,
}

impl<'a> TrackView<'a> {
    /// The numeric handle used to lookup this track's state buffers in the DSP engine.
    #[must_use]
    pub const fn id(self) -> TrackId {
        self.state.id()
    }

    /// The user-defined string name of the track.
    #[must_use]
    pub fn name(self) -> &'a str {
        self.state.name()
    }

    /// The origin of the audio signal (e.g., an evaluated `SamplePattern`).
    #[must_use]
    pub const fn source(self) -> &'a TrackSource {
        self.state.source()
    }

    /// The volume multiplier applied to the track's post-FX output.
    #[must_use]
    pub const fn level(self) -> f32 {
        self.state.level()
    }

    /// The stereo positioning of the track (-1.0 for left, 1.0 for right).
    #[must_use]
    pub const fn pan(self) -> f32 {
        self.state.pan()
    }

    /// Indicates whether the track has been explicitly silenced by the user.
    #[must_use]
    pub const fn muted(self) -> bool {
        self.state.muted()
    }

    /// Indicates that the track's signal is sent to the final output mix.
    #[must_use]
    pub const fn routes_to_master(self) -> bool {
        self.state.routes_to_master()
    }

    /// The quantity of auxiliary sends branching off this track.
    #[must_use]
    pub fn send_count(self) -> usize {
        self.state.sends.len()
    }

    /// A performance optimization flag indicating that no voices are currently active on this track,
    /// allowing the DSP engine to bypass mixing calculations entirely.
    #[must_use]
    pub fn is_phase1_silent(self) -> bool {
        self.state.is_phase1_silent()
    }
}

/// Read-only bus view exposed by a routing snapshot.
#[derive(Clone, Copy, Debug)]
pub struct BusView<'a> {
    state: &'a BusState,
}

impl<'a> BusView<'a> {
    /// The numeric handle used to lookup this bus's state buffers in the DSP engine.
    #[must_use]
    pub const fn id(self) -> BusId {
        self.state.id()
    }

    /// The user-defined string name of the bus.
    #[must_use]
    pub fn name(self) -> &'a str {
        self.state.name()
    }

    /// Indicates that the sum of the signals on this bus is sent to the final output mix.
    #[must_use]
    pub const fn routes_to_master(self) -> bool {
        self.state.routes_to_master()
    }

    /// The DSP configuration payload defining the shared effect applied to all signals traversing this bus.
    #[must_use]
    pub const fn effect(self) -> Option<&'a BusEffectSpec> {
        self.state.effect()
    }
}

#[derive(Clone, Debug)]
struct PendingTrack {
    name: Box<str>,
    source: TrackSource,
    level: f32,
    pan: f32,
    muted: bool,
    routes_to_master: bool,
}

#[derive(Clone, Debug)]
struct PendingBus {
    name: Box<str>,
}

#[derive(Clone, Debug)]
struct PendingBusEffect {
    bus_name: Box<str>,
    effect: BusEffectSpec,
}

#[derive(Clone, Debug)]
struct PendingSend {
    track_name: Box<str>,
    bus_name: Box<str>,
    level: f32,
}

#[derive(Clone, Debug)]
struct PendingRoute {
    from_name: Box<str>,
    to_name: Box<str>,
}

/// Errors raised while validating routing snapshots.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, Debug, Error, PartialEq)]
pub enum RoutingError {
    /// A track or bus name was defined multiple times.
    #[error("duplicate routing name '{name}'")]
    DuplicateName {
        /// The duplicated name.
        name: Box<str>,
    },
    /// A track was configured to send to the same bus multiple times.
    #[error("duplicate send from track '{track}' to bus '{bus}'")]
    DuplicateSend {
        /// The track name.
        track: Box<str>,
        /// The bus name.
        bus: Box<str>,
    },
    /// An operation referenced a track that does not exist.
    #[error("unknown track '{name}'")]
    UnknownTrack {
        /// The unknown track name.
        name: Box<str>,
    },
    /// An operation referenced a bus that does not exist.
    #[error("unknown bus '{name}'")]
    UnknownBus {
        /// The unknown bus name.
        name: Box<str>,
    },
    /// The routing configuration attempted to use a reserved internal name (like "master").
    #[error("reserved routing name '{name}' is not allowed")]
    ReservedName {
        /// The reserved name that was inappropriately used.
        name: Box<str>,
    },
    /// A send level was outside the valid range.
    #[error("invalid send level {level}; expected a finite value in [0, 1]")]
    InvalidLevel {
        /// The invalid level value.
        level: f32,
    },
    /// A track volume level was invalid (must be >= 0).
    #[error("invalid track level {level}; expected a finite value >= 0")]
    InvalidTrackLevel {
        /// The invalid track volume level.
        level: f32,
    },
    /// A track stereo pan setting was outside the valid bounds (-1.0 to 1.0).
    #[error("invalid track pan {pan}; expected a finite value in [-1, 1]")]
    InvalidPan {
        /// The invalid panning value.
        pan: f32,
    },
    /// A delay feedback amount was outside the valid bounds (0.0 to 1.0).
    #[error("invalid delay feedback {feedback}; expected a finite value in [0, 1]")]
    InvalidDelayFeedback {
        /// The invalid feedback value.
        feedback: f32,
    },
    /// A delay wet mix was outside the valid bounds (0.0 to 1.0).
    #[error("invalid delay wet {wet}; expected a finite value in [0, 1]")]
    InvalidDelayWet {
        /// The invalid wet mix value.
        wet: f32,
    },
    /// A delay time was negative or invalid.
    #[error("invalid delay time; expected a positive musical subdivision")]
    InvalidDelayTime,
    /// The specified reverb size is out of bounds (must be between 0.0 and 1.0).
    #[error("invalid reverb size {size}; expected a finite value in [0, 1]")]
    InvalidReverbSize {
        /// The invalid size value.
        size: f32,
    },
    /// The specified reverb damping amount is out of bounds (must be between 0.0 and 1.0).
    #[error("invalid reverb damp {damp}; expected a finite value in [0, 1]")]
    InvalidReverbDamp {
        /// The invalid damping value.
        damp: f32,
    },
    /// The specified reverb wet/dry mix is out of bounds (must be between 0.0 and 1.0).
    #[error("invalid reverb wet {wet}; expected a finite value in [0, 1]")]
    InvalidReverbWet {
        /// The invalid wet mix value.
        wet: f32,
    },
    /// A bus was assigned multiple effects, which is not supported.
    #[error("bus '{bus}' already hosts an effect")]
    DuplicateBusEffect {
        /// The name of the bus that received multiple effects.
        bus: Box<str>,
    },
    /// An attempt was made to directly route a track to a bus instead of using a send.
    #[error("track-to-bus routes must use send(...): '{from}' -> '{to}'")]
    TrackToBusRouteRequiresSend {
        /// The source track name.
        from: Box<str>,
        /// The destination bus name.
        to: Box<str>,
    },
    /// An attempt was made to route a bus into another bus, which could cause feedback loops.
    #[error("bus-to-bus routes are forbidden: '{from}' -> '{to}'")]
    BusToBusRoute {
        /// The source bus name.
        from: Box<str>,
        /// The destination bus name.
        to: Box<str>,
    },
    /// An attempt was made to route a bus back into a track, which violates the mixing hierarchy.
    #[error("bus-to-track routes are forbidden: '{from}' -> '{to}'")]
    BusToTrackRoute {
        /// The source bus name.
        from: Box<str>,
        /// The destination track name.
        to: Box<str>,
    },
    /// An attempt was made to route a track directly into another track, which violates the mixing hierarchy.
    #[error("track-to-track routes are forbidden: '{from}' -> '{to}'")]
    TrackToTrackRoute {
        /// The source track name.
        from: Box<str>,
        /// The destination track name.
        to: Box<str>,
    },
    /// The number of tracks or buses exceeded the limit that can be safely addressed.
    #[error("{kind} count exceeds the supported id range")]
    IdOverflow {
        /// The type of identifier that overflowed (e.g., "track" or "bus").
        kind: &'static str,
    },
}

/// Immutable routing snapshot consumed by the render thread.
#[derive(Clone, Debug, PartialEq)]
pub struct RoutingSnapshot {
    tracks: Box<[TrackState]>,
    buses: Box<[BusState]>,
    master_track_ids: Box<[TrackId]>,
    master_bus_ids: Box<[BusId]>,
}

impl RoutingSnapshot {
    /// Creates a new `RoutingSnapshotBuilder` for configuring mixer routing.
    #[must_use]
    pub fn builder() -> RoutingSnapshotBuilder {
        RoutingSnapshotBuilder::default()
    }

    /// The total count of parallel signal origins managed by the snapshot.
    #[must_use]
    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }

    /// The total count of shared effect processors managed by the snapshot.
    #[must_use]
    pub fn bus_count(&self) -> usize {
        self.buses.len()
    }

    /// Locates a track's properties and runtime state based on its user-assigned name.
    #[must_use]
    pub fn track(&self, name: &str) -> Option<TrackView<'_>> {
        self.tracks
            .iter()
            .find(|track| track.name() == name)
            .map(|state| TrackView { state })
    }

    /// Locates a bus's properties and runtime state based on its user-assigned name.
    #[must_use]
    pub fn bus(&self, name: &str) -> Option<BusView<'_>> {
        self.buses
            .iter()
            .find(|bus| bus.name() == name)
            .map(|state| BusView { state })
    }

    /// The list of tracks whose signals must be summed into the final output buffer.
    #[must_use]
    pub fn master_track_ids(&self) -> &[TrackId] {
        &self.master_track_ids
    }

    /// The list of buses whose signals must be summed into the final output buffer.
    #[must_use]
    pub fn master_bus_ids(&self) -> &[BusId] {
        &self.master_bus_ids
    }

    /// Read-only access to the sequential list of track states.
    #[must_use]
    pub fn tracks(&self) -> &[TrackState] {
        &self.tracks
    }

    /// Read-only access to the sequential list of bus states.
    #[must_use]
    pub fn buses(&self) -> &[BusState] {
        &self.buses
    }
}

/// Builder for a phase-1 routing snapshot.
#[derive(Clone, Debug, Default)]
pub struct RoutingSnapshotBuilder {
    tracks: Vec<PendingTrack>,
    buses: Vec<PendingBus>,
    bus_effects: Vec<PendingBusEffect>,
    sends: Vec<PendingSend>,
    routes: Vec<PendingRoute>,
}

impl RoutingSnapshotBuilder {
    /// Adds a new track to the routing snapshot with default parameters.
    #[must_use]
    pub fn track(self, name: impl Into<Box<str>>) -> Self {
        self.track_with_source(name, TrackSource::Unbound)
    }

    /// Adds a new track with a specific `TrackSource` (e.g., pattern-driven or direct).
    #[must_use]
    pub fn track_with_source(mut self, name: impl Into<Box<str>>, source: TrackSource) -> Self {
        self = self.track_with_source_and_mix(name, source, 1.0, 0.0, false);
        self
    }

    /// Adds a new track with fully specified mix parameters and source.
    #[must_use]
    pub fn track_with_source_and_mix(
        mut self,
        name: impl Into<Box<str>>,
        source: TrackSource,
        level: f32,
        pan: f32,
        muted: bool,
    ) -> Self {
        let name = name.into();
        self.tracks.push(PendingTrack {
            name,
            source,
            level,
            pan,
            muted,
            routes_to_master: false,
        });
        self
    }

    /// Adds the default "main" track used as the root fallback for patterns.
    #[must_use]
    pub fn main_track(self) -> Self {
        let mut builder = self.track_with_source("main", TrackSource::Unbound);
        if let Some(track) = builder.tracks.last_mut() {
            track.routes_to_master = true;
        }
        builder
    }

    /// Adds a new empty effect bus to the routing snapshot.
    #[must_use]
    pub fn bus(mut self, name: impl Into<Box<str>>) -> Self {
        self.buses.push(PendingBus { name: name.into() });
        self
    }

    /// Configures a bus to host a delay effect.
    #[must_use]
    pub fn bus_effect_delay(
        mut self,
        bus_name: impl Into<Box<str>>,
        time: Rational,
        feedback: f32,
        wet: f32,
    ) -> Self {
        self.bus_effects.push(PendingBusEffect {
            bus_name: bus_name.into(),
            effect: BusEffectSpec::Delay(DelaySpec::new(time, feedback, wet)),
        });
        self
    }

    /// Configures a bus to host a reverb effect.
    #[must_use]
    pub fn bus_effect_reverb(
        mut self,
        bus_name: impl Into<Box<str>>,
        size: f32,
        damp: f32,
        wet: f32,
    ) -> Self {
        self.bus_effects.push(PendingBusEffect {
            bus_name: bus_name.into(),
            effect: BusEffectSpec::Reverb(ReverbSpec::new(size, damp, wet)),
        });
        self
    }

    /// Creates an auxiliary send from a track to a bus.
    #[must_use]
    pub fn send(
        mut self,
        track_name: impl Into<Box<str>>,
        bus_name: impl Into<Box<str>>,
        level: f32,
    ) -> Self {
        self.sends.push(PendingSend {
            track_name: track_name.into(),
            bus_name: bus_name.into(),
            level,
        });
        self
    }

    /// Establishes a direct routing connection (e.g., routing a bus output to the master bus).
    #[must_use]
    pub fn route(mut self, from_name: impl Into<Box<str>>, to_name: impl Into<Box<str>>) -> Self {
        self.routes.push(PendingRoute {
            from_name: from_name.into(),
            to_name: to_name.into(),
        });
        self
    }

    /// Builds a validated immutable routing snapshot.
    ///
    /// # Errors
    ///
    /// Returns a [`RoutingError`] if the builder contains duplicate names,
    /// missing endpoints, invalid levels, or forbidden edge kinds.
    pub fn build(self) -> Result<RoutingSnapshot, RoutingError> {
        let Self {
            tracks: pending_tracks,
            buses: pending_buses,
            bus_effects: pending_bus_effects,
            sends: pending_sends,
            routes: pending_routes,
        } = self;

        let mut tracks = Self::materialize_tracks(pending_tracks)?;
        let mut buses = Self::materialize_buses(pending_buses, &tracks)?;
        Self::apply_bus_effects(pending_bus_effects, &mut buses)?;
        let track_sends = Self::materialize_sends(pending_sends, &tracks, &buses)?;
        Self::apply_routes(pending_routes, &mut tracks, &buses)?;

        let master_track_ids = tracks
            .iter()
            .filter(|track| track.routes_to_master())
            .map(TrackState::id)
            .collect::<Box<[_]>>();
        let master_bus_ids = buses
            .iter()
            .filter(|bus| bus.routes_to_master())
            .map(BusState::id)
            .collect::<Box<[_]>>();

        for (track, sends) in tracks.iter_mut().zip(track_sends) {
            track.sends = sends.into_boxed_slice();
        }

        Ok(RoutingSnapshot {
            tracks: tracks.into_boxed_slice(),
            buses: buses.into_boxed_slice(),
            master_track_ids,
            master_bus_ids,
        })
    }

    fn materialize_tracks(
        pending_tracks: Vec<PendingTrack>,
    ) -> Result<Vec<TrackState>, RoutingError> {
        let mut tracks = Vec::with_capacity(pending_tracks.len());

        for (track_index, pending_track) in pending_tracks.into_iter().enumerate() {
            ensure_not_master(&pending_track.name)?;
            ensure_name_is_unique(&pending_track.name, &tracks, &[])?;
            validate_track_level(pending_track.level)?;
            validate_pan(pending_track.pan)?;
            let track_id = track_id_from_index(track_index)?;

            tracks.push(TrackState::new(
                track_id,
                pending_track.name,
                pending_track.source,
                TrackMix {
                    level: pending_track.level,
                    pan: pending_track.pan,
                    muted: pending_track.muted,
                },
                pending_track.routes_to_master,
                Vec::new(),
            ));
        }

        Ok(tracks)
    }

    fn materialize_buses(
        pending_buses: Vec<PendingBus>,
        tracks: &[TrackState],
    ) -> Result<Vec<BusState>, RoutingError> {
        let mut buses = Vec::with_capacity(pending_buses.len());

        for (bus_index, pending_bus) in pending_buses.into_iter().enumerate() {
            ensure_not_master(&pending_bus.name)?;
            ensure_name_is_unique(&pending_bus.name, tracks, &buses)?;
            let bus_id = bus_id_from_index(bus_index)?;

            buses.push(BusState::new(bus_id, pending_bus.name));
        }

        Ok(buses)
    }

    fn materialize_sends(
        pending_sends: Vec<PendingSend>,
        tracks: &[TrackState],
        buses: &[BusState],
    ) -> Result<Vec<Vec<SendRoute>>, RoutingError> {
        let mut track_sends = vec![Vec::new(); tracks.len()];

        for pending_send in pending_sends {
            validate_level(pending_send.level)?;

            let track_index = track_index(tracks, &pending_send.track_name).ok_or_else(|| {
                RoutingError::UnknownTrack {
                    name: pending_send.track_name.clone(),
                }
            })?;
            let bus_index = bus_index(buses, &pending_send.bus_name).ok_or_else(|| {
                RoutingError::UnknownBus {
                    name: pending_send.bus_name.clone(),
                }
            })?;

            push_send(
                &mut track_sends[track_index],
                tracks[track_index].name(),
                buses[bus_index].id(),
                pending_send.level,
                pending_send.bus_name,
            )?;
        }

        Ok(track_sends)
    }

    fn apply_bus_effects(
        pending_bus_effects: Vec<PendingBusEffect>,
        buses: &mut [BusState],
    ) -> Result<(), RoutingError> {
        for pending_bus_effect in pending_bus_effects {
            let bus_index = bus_index(buses, &pending_bus_effect.bus_name).ok_or_else(|| {
                RoutingError::UnknownBus {
                    name: pending_bus_effect.bus_name.clone(),
                }
            })?;

            if buses[bus_index].effect.is_some() {
                return Err(RoutingError::DuplicateBusEffect {
                    bus: pending_bus_effect.bus_name,
                });
            }

            validate_bus_effect(&pending_bus_effect.effect)?;
            buses[bus_index].effect = Some(pending_bus_effect.effect);
        }

        Ok(())
    }

    fn apply_routes(
        pending_routes: Vec<PendingRoute>,
        tracks: &mut [TrackState],
        buses: &[BusState],
    ) -> Result<(), RoutingError> {
        for pending_route in pending_routes {
            if pending_route.to_name.as_ref() == "master" {
                route_to_master(&pending_route, tracks, buses)?;
                continue;
            }

            let from_track = track_index(tracks, &pending_route.from_name);
            let from_bus = bus_index(buses, &pending_route.from_name);
            let to_track = track_index(tracks, &pending_route.to_name);
            let to_bus = bus_index(buses, &pending_route.to_name);

            if from_track.is_some() {
                if to_bus.is_some() {
                    return Err(RoutingError::TrackToBusRouteRequiresSend {
                        from: pending_route.from_name,
                        to: pending_route.to_name,
                    });
                } else if to_track.is_some() {
                    return Err(RoutingError::TrackToTrackRoute {
                        from: pending_route.from_name,
                        to: pending_route.to_name,
                    });
                }
                return Err(RoutingError::UnknownBus {
                    name: pending_route.to_name,
                });
            } else if let Some(_bus_index) = from_bus {
                if to_bus.is_some() {
                    return Err(RoutingError::BusToBusRoute {
                        from: pending_route.from_name,
                        to: pending_route.to_name,
                    });
                } else if to_track.is_some() {
                    return Err(RoutingError::BusToTrackRoute {
                        from: pending_route.from_name,
                        to: pending_route.to_name,
                    });
                }
                return Err(RoutingError::UnknownBus {
                    name: pending_route.to_name,
                });
            }
            return Err(RoutingError::UnknownTrack {
                name: pending_route.from_name,
            });
        }

        Ok(())
    }
}

fn validate_level(level: f32) -> Result<(), RoutingError> {
    if level.is_finite() && (0.0..=1.0).contains(&level) {
        Ok(())
    } else {
        Err(RoutingError::InvalidLevel { level })
    }
}

fn validate_track_level(level: f32) -> Result<(), RoutingError> {
    if level.is_finite() && level >= 0.0 {
        Ok(())
    } else {
        Err(RoutingError::InvalidTrackLevel { level })
    }
}

fn validate_pan(pan: f32) -> Result<(), RoutingError> {
    if pan.is_finite() && (-1.0..=1.0).contains(&pan) {
        Ok(())
    } else {
        Err(RoutingError::InvalidPan { pan })
    }
}

fn validate_bus_effect(effect: &BusEffectSpec) -> Result<(), RoutingError> {
    match effect {
        BusEffectSpec::Delay(spec) => validate_delay_spec(spec),
        BusEffectSpec::Reverb(spec) => validate_reverb_spec(spec),
    }
}

fn validate_delay_spec(spec: &DelaySpec) -> Result<(), RoutingError> {
    validate_wet_scalar(
        spec.feedback(),
        RoutingError::InvalidDelayFeedback {
            feedback: spec.feedback(),
        },
    )?;
    validate_wet_scalar(
        spec.wet(),
        RoutingError::InvalidDelayWet { wet: spec.wet() },
    )?;
    if spec.time() <= &Rational::zero() {
        return Err(RoutingError::InvalidDelayTime);
    }
    Ok(())
}

fn validate_reverb_spec(spec: &ReverbSpec) -> Result<(), RoutingError> {
    validate_wet_scalar(
        spec.size(),
        RoutingError::InvalidReverbSize { size: spec.size() },
    )?;
    validate_wet_scalar(
        spec.damp(),
        RoutingError::InvalidReverbDamp { damp: spec.damp() },
    )?;
    validate_wet_scalar(
        spec.wet(),
        RoutingError::InvalidReverbWet { wet: spec.wet() },
    )?;
    Ok(())
}

fn validate_wet_scalar(value: f32, error: RoutingError) -> Result<(), RoutingError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(error)
    }
}

fn ensure_not_master(name: &str) -> Result<(), RoutingError> {
    if name == "master" {
        return Err(RoutingError::ReservedName {
            name: Box::<str>::from(name),
        });
    }

    Ok(())
}

fn ensure_name_is_unique(
    name: &str,
    tracks: &[TrackState],
    buses: &[BusState],
) -> Result<(), RoutingError> {
    if tracks.iter().any(|track| track.name() == name) || buses.iter().any(|bus| bus.name() == name)
    {
        return Err(RoutingError::DuplicateName {
            name: Box::<str>::from(name),
        });
    }

    Ok(())
}

fn track_id_from_index(index: usize) -> Result<TrackId, RoutingError> {
    let value = u32::try_from(index).map_err(|_| RoutingError::IdOverflow { kind: "track" })?;
    Ok(TrackId::new(value))
}

fn bus_id_from_index(index: usize) -> Result<BusId, RoutingError> {
    let value = u32::try_from(index).map_err(|_| RoutingError::IdOverflow { kind: "bus" })?;
    Ok(BusId::new(value))
}

fn track_index(tracks: &[TrackState], name: &str) -> Option<usize> {
    tracks.iter().position(|track| track.name() == name)
}

fn bus_index(buses: &[BusState], name: &str) -> Option<usize> {
    buses.iter().position(|bus| bus.name() == name)
}

fn push_send(
    sends: &mut Vec<SendRoute>,
    track_name: &str,
    bus_id: BusId,
    level: f32,
    bus_name: Box<str>,
) -> Result<(), RoutingError> {
    if sends.iter().any(|send| send.bus_id() == bus_id) {
        return Err(RoutingError::DuplicateSend {
            track: Box::<str>::from(track_name),
            bus: bus_name,
        });
    }

    sends.push(SendRoute::new(bus_id, level));
    Ok(())
}

fn route_to_master(
    route: &PendingRoute,
    tracks: &mut [TrackState],
    buses: &[BusState],
) -> Result<(), RoutingError> {
    if let Some(track_index) = track_index(tracks, &route.from_name) {
        tracks[track_index].routes_to_master = true;
        return Ok(());
    }

    if bus_index(buses, &route.from_name).is_some() {
        return Ok(());
    }

    Err(RoutingError::UnknownTrack {
        name: route.from_name.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routing_snapshot_builder_catches_duplicate_track_names() {
        let builder = RoutingSnapshotBuilder::default()
            .track("drums")
            .track("drums");
        let result = builder.build();
        assert_eq!(
            result,
            Err(RoutingError::DuplicateName {
                name: Box::<str>::from("drums")
            })
        );
    }

    #[test]
    fn routing_snapshot_builder_catches_duplicate_bus_names() {
        let builder = RoutingSnapshotBuilder::default().bus("verb").bus("verb");
        let result = builder.build();
        assert_eq!(
            result,
            Err(RoutingError::DuplicateName {
                name: Box::<str>::from("verb")
            })
        );
    }

    #[test]
    fn routing_snapshot_builder_catches_track_and_bus_name_collisions() {
        let builder = RoutingSnapshotBuilder::default()
            .track("shared")
            .bus("shared");
        let result = builder.build();
        assert_eq!(
            result,
            Err(RoutingError::DuplicateName {
                name: Box::<str>::from("shared")
            })
        );
    }

    #[test]
    fn routing_snapshot_builder_catches_reserved_master_name_for_tracks() {
        let builder = RoutingSnapshotBuilder::default().track("master");
        let result = builder.build();
        assert_eq!(
            result,
            Err(RoutingError::ReservedName {
                name: Box::<str>::from("master")
            })
        );
    }

    #[test]
    fn routing_snapshot_builder_catches_reserved_master_name_for_buses() {
        let builder = RoutingSnapshotBuilder::default().bus("master");
        let result = builder.build();
        assert_eq!(
            result,
            Err(RoutingError::ReservedName {
                name: Box::<str>::from("master")
            })
        );
    }

    #[test]
    fn routing_snapshot_builder_catches_unknown_track_in_send() {
        let builder = RoutingSnapshotBuilder::default()
            .bus("verb")
            .send("drums", "verb", 1.0);
        let result = builder.build();
        assert_eq!(
            result,
            Err(RoutingError::UnknownTrack {
                name: Box::<str>::from("drums")
            })
        );
    }

    #[test]
    fn routing_snapshot_builder_catches_unknown_bus_in_send() {
        let builder = RoutingSnapshotBuilder::default()
            .track("drums")
            .send("drums", "verb", 1.0);
        let result = builder.build();
        assert_eq!(
            result,
            Err(RoutingError::UnknownBus {
                name: Box::<str>::from("verb")
            })
        );
    }

    #[test]
    fn routing_snapshot_builder_catches_duplicate_send() {
        let builder = RoutingSnapshotBuilder::default()
            .track("drums")
            .bus("verb")
            .send("drums", "verb", 0.5)
            .send("drums", "verb", 0.8);
        let result = builder.build();
        assert_eq!(
            result,
            Err(RoutingError::DuplicateSend {
                track: Box::<str>::from("drums"),
                bus: Box::<str>::from("verb")
            })
        );
    }

    #[test]
    fn routing_snapshot_builder_catches_track_to_track_route() {
        let builder = RoutingSnapshotBuilder::default()
            .track("drums")
            .track("comp")
            .route("drums", "comp");
        let result = builder.build();
        assert_eq!(
            result,
            Err(RoutingError::TrackToTrackRoute {
                from: Box::<str>::from("drums"),
                to: Box::<str>::from("comp")
            })
        );
    }

    #[test]
    fn routing_snapshot_builder_catches_bus_to_bus_route() {
        let builder = RoutingSnapshotBuilder::default()
            .bus("delay")
            .bus("verb")
            .route("delay", "verb");
        let result = builder.build();
        assert_eq!(
            result,
            Err(RoutingError::BusToBusRoute {
                from: Box::<str>::from("delay"),
                to: Box::<str>::from("verb")
            })
        );
    }

    #[test]
    fn routing_snapshot_builder_catches_bus_to_track_route() {
        let builder = RoutingSnapshotBuilder::default()
            .track("drums")
            .bus("verb")
            .route("verb", "drums");
        let result = builder.build();
        assert_eq!(
            result,
            Err(RoutingError::BusToTrackRoute {
                from: Box::<str>::from("verb"),
                to: Box::<str>::from("drums")
            })
        );
    }

    #[test]
    fn routing_snapshot_builder_catches_track_to_bus_route() {
        let builder = RoutingSnapshotBuilder::default()
            .track("drums")
            .bus("verb")
            .route("drums", "verb");
        let result = builder.build();
        assert_eq!(
            result,
            Err(RoutingError::TrackToBusRouteRequiresSend {
                from: Box::<str>::from("drums"),
                to: Box::<str>::from("verb")
            })
        );
    }
}
