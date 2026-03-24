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

use crate::SampleTrigger;

/// Stable identifier for a track in a routing snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackId(u32);

impl TrackId {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

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
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

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
}

impl TrackSource {
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
    Delay(DelaySpec),
    Reverb(ReverbSpec),
}

impl BusEffectSpec {
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
    #[must_use]
    pub const fn new(time: Rational, feedback: f32, wet: f32) -> Self {
        Self {
            time,
            feedback,
            wet,
        }
    }

    #[must_use]
    pub const fn time(&self) -> &Rational {
        &self.time
    }

    #[must_use]
    pub const fn feedback(&self) -> f32 {
        self.feedback
    }

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
    #[must_use]
    pub const fn new(size: f32, damp: f32, wet: f32) -> Self {
        Self { size, damp, wet }
    }

    #[must_use]
    pub const fn size(&self) -> f32 {
        self.size
    }

    #[must_use]
    pub const fn damp(&self) -> f32 {
        self.damp
    }

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
    #[must_use]
    pub const fn id(self) -> TrackId {
        self.state.id()
    }

    #[must_use]
    pub fn name(self) -> &'a str {
        self.state.name()
    }

    #[must_use]
    pub const fn source(self) -> &'a TrackSource {
        self.state.source()
    }

    #[must_use]
    pub const fn level(self) -> f32 {
        self.state.level()
    }

    #[must_use]
    pub const fn pan(self) -> f32 {
        self.state.pan()
    }

    #[must_use]
    pub const fn muted(self) -> bool {
        self.state.muted()
    }

    #[must_use]
    pub const fn routes_to_master(self) -> bool {
        self.state.routes_to_master()
    }

    #[must_use]
    pub fn send_count(self) -> usize {
        self.state.sends.len()
    }

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
    #[must_use]
    pub const fn id(self) -> BusId {
        self.state.id()
    }

    #[must_use]
    pub fn name(self) -> &'a str {
        self.state.name()
    }

    #[must_use]
    pub const fn routes_to_master(self) -> bool {
        self.state.routes_to_master()
    }

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
    #[error("duplicate routing name '{name}'")]
    DuplicateName { name: Box<str> },
    #[error("duplicate send from track '{track}' to bus '{bus}'")]
    DuplicateSend { track: Box<str>, bus: Box<str> },
    #[error("unknown track '{name}'")]
    UnknownTrack { name: Box<str> },
    #[error("unknown bus '{name}'")]
    UnknownBus { name: Box<str> },
    #[error("reserved routing name '{name}' is not allowed")]
    ReservedName { name: Box<str> },
    #[error("invalid send level {level}; expected a finite value in [0, 1]")]
    InvalidLevel { level: f32 },
    #[error("invalid track level {level}; expected a finite value >= 0")]
    InvalidTrackLevel { level: f32 },
    #[error("invalid track pan {pan}; expected a finite value in [-1, 1]")]
    InvalidPan { pan: f32 },
    #[error("invalid delay feedback {feedback}; expected a finite value in [0, 1]")]
    InvalidDelayFeedback { feedback: f32 },
    #[error("invalid delay wet {wet}; expected a finite value in [0, 1]")]
    InvalidDelayWet { wet: f32 },
    #[error("invalid delay time; expected a positive musical subdivision")]
    InvalidDelayTime,
    #[error("invalid reverb size {size}; expected a finite value in [0, 1]")]
    InvalidReverbSize { size: f32 },
    #[error("invalid reverb damp {damp}; expected a finite value in [0, 1]")]
    InvalidReverbDamp { damp: f32 },
    #[error("invalid reverb wet {wet}; expected a finite value in [0, 1]")]
    InvalidReverbWet { wet: f32 },
    #[error("bus '{bus}' already hosts an effect")]
    DuplicateBusEffect { bus: Box<str> },
    #[error("track-to-bus routes must use send(...): '{from}' -> '{to}'")]
    TrackToBusRouteRequiresSend { from: Box<str>, to: Box<str> },
    #[error("bus-to-bus routes are forbidden: '{from}' -> '{to}'")]
    BusToBusRoute { from: Box<str>, to: Box<str> },
    #[error("bus-to-track routes are forbidden: '{from}' -> '{to}'")]
    BusToTrackRoute { from: Box<str>, to: Box<str> },
    #[error("track-to-track routes are forbidden: '{from}' -> '{to}'")]
    TrackToTrackRoute { from: Box<str>, to: Box<str> },
    #[error("{kind} count exceeds the supported id range")]
    IdOverflow { kind: &'static str },
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
    #[must_use]
    pub fn builder() -> RoutingSnapshotBuilder {
        RoutingSnapshotBuilder::default()
    }

    #[must_use]
    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }

    #[must_use]
    pub fn bus_count(&self) -> usize {
        self.buses.len()
    }

    #[must_use]
    pub fn track(&self, name: &str) -> Option<TrackView<'_>> {
        self.tracks
            .iter()
            .find(|track| track.name() == name)
            .map(|state| TrackView { state })
    }

    #[must_use]
    pub fn bus(&self, name: &str) -> Option<BusView<'_>> {
        self.buses
            .iter()
            .find(|bus| bus.name() == name)
            .map(|state| BusView { state })
    }

    #[must_use]
    pub fn master_track_ids(&self) -> &[TrackId] {
        &self.master_track_ids
    }

    #[must_use]
    pub fn master_bus_ids(&self) -> &[BusId] {
        &self.master_bus_ids
    }

    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn tracks(&self) -> &[TrackState] {
        &self.tracks
    }

    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn buses(&self) -> &[BusState] {
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
    #[must_use]
    pub fn track(self, name: impl Into<Box<str>>) -> Self {
        self.track_with_source(name, TrackSource::Unbound)
    }

    #[must_use]
    pub fn track_with_source(mut self, name: impl Into<Box<str>>, source: TrackSource) -> Self {
        self = self.track_with_source_and_mix(name, source, 1.0, 0.0, false);
        self
    }

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

    #[must_use]
    pub fn main_track(self) -> Self {
        let mut builder = self.track_with_source("main", TrackSource::Unbound);
        if let Some(track) = builder.tracks.last_mut() {
            track.routes_to_master = true;
        }
        builder
    }

    #[must_use]
    pub fn bus(mut self, name: impl Into<Box<str>>) -> Self {
        self.buses.push(PendingBus { name: name.into() });
        self
    }

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
            .collect::<Vec<_>>();
        let master_bus_ids = buses
            .iter()
            .filter(|bus| bus.routes_to_master())
            .map(BusState::id)
            .collect::<Vec<_>>();

        for (track, sends) in tracks.iter_mut().zip(track_sends) {
            track.sends = sends.into_boxed_slice();
        }

        Ok(RoutingSnapshot {
            tracks: tracks.into_boxed_slice(),
            buses: buses.into_boxed_slice(),
            master_track_ids: master_track_ids.into_boxed_slice(),
            master_bus_ids: master_bus_ids.into_boxed_slice(),
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
