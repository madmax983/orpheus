//! The `effect_bus` module provides named send/return effect buses.
//!
//! An [`EffectBus`] accumulates audio from voices via send routing, processes
//! the accumulated signal through an effect chain, and mixes the wet result
//! back into the master output. Bus buffers are pre-allocated at construction
//! time to remain allocation-free during real-time rendering.

use crate::effect::Effect;

/// Maximum number of stereo frames per render buffer.
///
/// This sets the upper bound for pre-allocated bus send buffers. Typical
/// audio callbacks use 256-2048 frames; 8192 provides generous headroom.
const MAX_BUFFER_FRAMES: usize = 8192;

/// A named send/return effect bus with pre-allocated buffers.
#[derive(Debug)]
pub struct EffectBus {
    name: Box<str>,
    /// Pre-allocated interleaved stereo send buffer (L, R, L, R, ...).
    send_buffer: Vec<f32>,
    effect: Box<dyn Effect>,
    return_level: f32,
}

impl EffectBus {
    /// Creates a new effect bus with pre-allocated buffers.
    #[must_use]
    pub fn new(name: impl Into<Box<str>>, effect: Box<dyn Effect>, return_level: f32) -> Self {
        Self {
            name: name.into(),
            send_buffer: vec![0.0; MAX_BUFFER_FRAMES * 2],
            effect,
            return_level: return_level.clamp(0.0, 1.0),
        }
    }

    /// Returns the bus name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the return level.
    #[must_use]
    pub const fn return_level(&self) -> f32 {
        self.return_level
    }

    /// Clears the send buffer for the next render block.
    pub fn clear_send_buffer(&mut self, frame_count: usize) {
        let sample_count = (frame_count * 2).min(self.send_buffer.len());
        self.send_buffer[..sample_count].fill(0.0);
    }

    /// Accumulates a stereo sample pair into the send buffer at the given frame index.
    #[allow(clippy::cast_possible_truncation)]
    pub fn accumulate(&mut self, frame_index: usize, left: f32, right: f32, send_level: f32) {
        let idx = frame_index * 2;
        if idx + 1 < self.send_buffer.len() {
            self.send_buffer[idx] += left * send_level;
            self.send_buffer[idx + 1] += right * send_level;
        }
    }

    /// Processes the accumulated send buffer through the effect chain in-place,
    /// then returns a slice of the processed stereo buffer.
    pub fn process(&mut self, frame_count: usize) {
        let sample_count = (frame_count * 2).min(self.send_buffer.len());
        self.effect
            .process_stereo(&mut self.send_buffer[..sample_count]);
    }

    /// Returns the processed wet signal at the given frame index, scaled by return level.
    #[must_use]
    pub fn wet_frame(&self, frame_index: usize) -> (f32, f32) {
        let idx = frame_index * 2;
        if idx + 1 < self.send_buffer.len() {
            (
                self.send_buffer[idx] * self.return_level,
                self.send_buffer[idx + 1] * self.return_level,
            )
        } else {
            (0.0, 0.0)
        }
    }

    /// Resets the effect's internal state (e.g. on transport stop).
    pub fn reset(&mut self) {
        self.effect.reset();
        self.send_buffer.fill(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::StereoDelay;

    #[test]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn accumulate_and_process_produces_wet_output() {
        let delay = StereoDelay::new(48_000, 0.01, 0.0, 1.0);
        let mut bus = EffectBus::new("test_delay", Box::new(delay), 1.0);
        let delay_frames = (48_000.0 * 0.01_f64).ceil() as usize;

        let total_frames = delay_frames * 3;
        bus.clear_send_buffer(total_frames);

        // Accumulate an impulse at frame 0
        bus.accumulate(0, 1.0, 1.0, 1.0);

        // Process the buffer
        bus.process(total_frames);

        // Dry impulse + no wet at frame 0 (delay hasn't occurred yet)
        let (l0, _r0) = bus.wet_frame(0);
        assert!(l0.abs() > 0.9, "processed impulse at frame 0: {l0}");

        // Echo at delay offset
        let (l_echo, r_echo) = bus.wet_frame(delay_frames);
        assert!(l_echo.abs() > 0.9, "echo left at {delay_frames}: {l_echo}");
        assert!(r_echo.abs() > 0.9, "echo right at {delay_frames}: {r_echo}");
    }

    #[test]
    fn zero_return_level_silences_output() {
        let delay = StereoDelay::new(48_000, 0.01, 0.0, 1.0);
        let mut bus = EffectBus::new("silent", Box::new(delay), 0.0);

        bus.clear_send_buffer(1000);
        bus.accumulate(0, 1.0, 1.0, 1.0);
        bus.process(1000);

        for i in 0..1000 {
            let (l, r) = bus.wet_frame(i);
            assert!(
                l.abs() < f32::EPSILON && r.abs() < f32::EPSILON,
                "frame {i} should be silent with return_level=0"
            );
        }
    }
}
