//! Tempo-synchronized delay effect.
//!
//! This module implements a stereo delay line that calculates its frame offsets
//! exactly using `Rational` cycle times rather than generic milliseconds. This
//! ensures that delay taps always land precisely on musical grid subdivisions
//! without drifting.

use orpheus_pattern::Rational;

use crate::engine::EngineError;
use crate::routing::DelaySpec;

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
