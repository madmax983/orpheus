#![allow(clippy::float_cmp)]
//! A tempo-synchronized stereo delay effect.
//!
//! This delay uses a pre-allocated circular buffer to store previous audio frames.
//!
//! # Time Synchronization
//! The delay time is not specified in milliseconds, but as an exact [`Rational`]
//! value representing a fraction of a cycle (e.g., `1/4` for a quarter-cycle delay).
//! This is converted into an exact number of audio frames based on the current
//! `frames_per_cycle` provided by the DSP scheduler. If the tempo changes, the
//! buffer size is re-calculated and re-allocated via [`DelayState::sync_timing`].

use orpheus_pattern::Rational;

use crate::engine::EngineError;
use crate::routing::DelaySpec;

/// A tempo-synchronized stereo delay effect.
///
/// This delay uses a pre-allocated circular buffer to store previous audio frames.
///
/// # Time Synchronization
/// The delay time is not specified in milliseconds, but as an exact [`Rational`]
/// value representing a fraction of a cycle (e.g., `1/4` for a quarter-cycle delay).
/// This is converted into an exact number of audio frames based on the current
/// `frames_per_cycle` provided by the DSP scheduler. If the tempo changes, the
/// buffer size is re-calculated and re-allocated via [`DelayState::sync_timing`].
#[derive(Debug)]
pub struct DelayState {
    buffer: Vec<(f32, f32)>,
    write_index: usize,
    delay_frames: usize,
    feedback: f32,
    wet: f32,
}

impl DelayState {
    pub fn new(spec: &DelaySpec, frames_per_cycle: u64) -> Result<Self, EngineError> {
        let delay_frames = delay_frames(spec.time(), frames_per_cycle)?;
        Ok(Self {
            buffer: vec![(0.0, 0.0); delay_frames],
            write_index: 0,
            delay_frames,
            feedback: spec.feedback(),
            wet: spec.wet(),
        })
    }

    pub fn sync_timing(
        &mut self,
        spec: &DelaySpec,
        frames_per_cycle: u64,
    ) -> Result<(), EngineError> {
        let delay_frames = delay_frames(spec.time(), frames_per_cycle)?;
        self.feedback = spec.feedback();
        self.wet = spec.wet();
        if delay_frames != self.delay_frames {
            self.buffer = vec![(0.0, 0.0); delay_frames];
            self.delay_frames = delay_frames;
            self.write_index = 0;
        }
        Ok(())
    }

    #[must_use]
    pub fn process_frame(&mut self, input_left: f32, input_right: f32) -> (f32, f32) {
        let (delayed_left, delayed_right) = self.buffer[self.write_index];
        self.buffer[self.write_index] = (
            delayed_left.mul_add(self.feedback, input_left),
            delayed_right.mul_add(self.feedback, input_right),
        );
        self.write_index += 1;
        if self.write_index == self.delay_frames {
            self.write_index = 0;
        }
        (delayed_left * self.wet, delayed_right * self.wet)
    }

    pub fn reset(&mut self) {
        self.buffer.fill((0.0, 0.0));
        self.write_index = 0;
    }
}

fn delay_frames(time: &Rational, frames_per_cycle: u64) -> Result<usize, EngineError> {
    let scaled = time
        .numerator()
        .checked_mul(i128::from(frames_per_cycle))
        .ok_or(EngineError::FrameOverflow)?;
    let frames = scaled / time.denominator();
    if frames <= 0 {
        return Err(EngineError::FrameOverflow);
    }
    usize::try_from(frames).map_err(|_| EngineError::FrameOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_calculate_delay_frames_correctly() {
        let time = Rational::new(1, 2).unwrap();
        let frames_per_cycle = 44100;
        assert_eq!(delay_frames(&time, frames_per_cycle).unwrap(), 22050);
    }

    #[test]
    fn should_return_error_when_time_is_zero() {
        let time = Rational::new(0, 1).unwrap();
        let frames_per_cycle = 44100;
        assert!(matches!(
            delay_frames(&time, frames_per_cycle),
            Err(EngineError::FrameOverflow)
        ));
    }

    #[test]
    fn should_return_error_when_time_is_negative() {
        let time = Rational::new(-1, 2).unwrap();
        let frames_per_cycle = 44100;
        assert!(matches!(
            delay_frames(&time, frames_per_cycle),
            Err(EngineError::FrameOverflow)
        ));
    }

    #[test]
    fn should_initialize_delay_state_correctly() {
        let spec = DelaySpec::new(Rational::new(1, 4).unwrap(), 0.5, 0.2);
        let state = DelayState::new(&spec, 44100).unwrap();
        assert_eq!(state.buffer.len(), 11025);
        assert_eq!(state.write_index, 0);
        assert_eq!(state.delay_frames, 11025);
        assert_eq!(state.feedback, 0.5);
        assert_eq!(state.wet, 0.2);
    }

    #[test]
    fn should_sync_timing_correctly() {
        let spec1 = DelaySpec::new(Rational::new(1, 4).unwrap(), 0.5, 0.2);
        let mut state = DelayState::new(&spec1, 44100).unwrap();

        let spec2 = DelaySpec::new(Rational::new(1, 2).unwrap(), 0.7, 0.3);
        state.sync_timing(&spec2, 44100).unwrap();

        assert_eq!(state.buffer.len(), 22050);
        assert_eq!(state.write_index, 0);
        assert_eq!(state.delay_frames, 22050);
        assert_eq!(state.feedback, 0.7);
        assert_eq!(state.wet, 0.3);
    }

    #[test]
    fn should_process_frame_correctly() {
        let spec = DelaySpec::new(Rational::new(1, 44100).unwrap(), 0.5, 1.0); // 1 frame delay
        let mut state = DelayState::new(&spec, 44100).unwrap();

        // Frame 1
        let out1 = state.process_frame(1.0, -1.0);
        assert_eq!(out1, (0.0, 0.0));

        // Frame 2
        let out2 = state.process_frame(0.0, 0.0);
        assert_eq!(out2, (1.0, -1.0));

        // Frame 3 (Feedback)
        let out3 = state.process_frame(0.0, 0.0);
        assert_eq!(out3, (0.5, -0.5));
    }

    #[test]
    fn should_reset_state_correctly() {
        let spec = DelaySpec::new(Rational::new(1, 4).unwrap(), 0.5, 0.2);
        let mut state = DelayState::new(&spec, 44100).unwrap();

        let _ = state.process_frame(1.0, 1.0);
        state.reset();

        assert_eq!(state.write_index, 0);
        assert_eq!(state.buffer[0], (0.0, 0.0));
    }
}
