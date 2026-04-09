#![allow(clippy::float_cmp)]
//! An algorithmic stereo reverberator.
//!
//! This reverb is constructed from parallel comb filters fed into a series of
//! allpass filters. The hardcoded delay lengths are tuned slightly differently
//! for the left and right channels to create a wide stereo image from a mono input.

use crate::routing::ReverbSpec;

const LEFT_COMB_LENGTHS: [usize; 4] = [149, 211, 263, 293];
const RIGHT_COMB_LENGTHS: [usize; 4] = [163, 223, 277, 307];
const LEFT_ALLPASS_LENGTHS: [usize; 2] = [43, 17];
const RIGHT_ALLPASS_LENGTHS: [usize; 2] = [47, 19];
const ALLPASS_FEEDBACK: f32 = 0.5;
const INPUT_GAIN: f32 = 0.125;

#[derive(Debug)]
pub struct ReverbState {
    left: ReverbChannelState,
    right: ReverbChannelState,
    wet: f32,
}

impl ReverbState {
    #[must_use]
    pub fn new(spec: &ReverbSpec) -> Self {
        let feedback = reverb_feedback(spec.size());
        let damp = spec.damp();
        Self {
            left: ReverbChannelState::new(
                &LEFT_COMB_LENGTHS,
                &LEFT_ALLPASS_LENGTHS,
                feedback,
                damp,
            ),
            right: ReverbChannelState::new(
                &RIGHT_COMB_LENGTHS,
                &RIGHT_ALLPASS_LENGTHS,
                feedback,
                damp,
            ),
            wet: spec.wet(),
        }
    }

    pub fn sync_spec(&mut self, spec: &ReverbSpec) {
        let feedback = reverb_feedback(spec.size());
        self.left.sync_spec(feedback, spec.damp());
        self.right.sync_spec(feedback, spec.damp());
        self.wet = spec.wet();
    }

    #[must_use]
    pub fn process_frame(&mut self, input_left: f32, input_right: f32) -> (f32, f32) {
        let input = (input_left + input_right) * INPUT_GAIN;
        let left = self.left.process(input) * self.wet;
        let right = self.right.process(input) * self.wet;
        (left, right)
    }

    pub fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
    }
}

#[derive(Debug)]
struct ReverbChannelState {
    combs: Vec<CombState>,
    allpasses: Vec<AllpassState>,
}

impl ReverbChannelState {
    fn new(comb_lengths: &[usize], allpass_lengths: &[usize], feedback: f32, damp: f32) -> Self {
        Self {
            combs: comb_lengths
                .iter()
                .map(|&length| CombState::new(length, feedback, damp))
                .collect(),
            allpasses: allpass_lengths
                .iter()
                .map(|&length| AllpassState::new(length, ALLPASS_FEEDBACK))
                .collect(),
        }
    }

    fn sync_spec(&mut self, feedback: f32, damp: f32) {
        for comb in &mut self.combs {
            comb.sync_spec(feedback, damp);
        }
    }

    #[must_use]
    fn process(&mut self, input: f32) -> f32 {
        let mut sample = self
            .combs
            .iter_mut()
            .fold(0.0_f32, |acc, comb| acc + comb.process(input));
        for allpass in &mut self.allpasses {
            sample = allpass.process(sample);
        }
        sample
    }

    fn reset(&mut self) {
        for comb in &mut self.combs {
            comb.reset();
        }
        for allpass in &mut self.allpasses {
            allpass.reset();
        }
    }
}

#[derive(Debug)]
struct CombState {
    buffer: Vec<f32>,
    index: usize,
    feedback: f32,
    damp: f32,
    filter_store: f32,
}

impl CombState {
    fn new(length: usize, feedback: f32, damp: f32) -> Self {
        Self {
            buffer: vec![0.0; length],
            index: 0,
            feedback,
            damp,
            filter_store: 0.0,
        }
    }

    const fn sync_spec(&mut self, feedback: f32, damp: f32) {
        self.feedback = feedback;
        self.damp = damp;
    }

    #[must_use]
    fn process(&mut self, input: f32) -> f32 {
        let output = self.buffer[self.index];
        self.filter_store = output.mul_add(1.0 - self.damp, self.filter_store * self.damp);
        self.buffer[self.index] = self.filter_store.mul_add(self.feedback, input);
        self.index += 1;
        if self.index == self.buffer.len() {
            self.index = 0;
        }
        output
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.index = 0;
        self.filter_store = 0.0;
    }
}

#[derive(Debug)]
struct AllpassState {
    buffer: Vec<f32>,
    index: usize,
    feedback: f32,
}

impl AllpassState {
    fn new(length: usize, feedback: f32) -> Self {
        Self {
            buffer: vec![0.0; length],
            index: 0,
            feedback,
        }
    }

    #[must_use]
    fn process(&mut self, input: f32) -> f32 {
        let buffered = self.buffer[self.index];
        let output = buffered - input;
        self.buffer[self.index] = buffered.mul_add(self.feedback, input);
        self.index += 1;
        if self.index == self.buffer.len() {
            self.index = 0;
        }
        output
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.index = 0;
    }
}

#[must_use]
fn reverb_feedback(size: f32) -> f32 {
    size.mul_add(0.55, 0.35)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_calculate_reverb_feedback() {
        assert_eq!(reverb_feedback(0.0), 0.35);
        assert_eq!(reverb_feedback(1.0), 0.9);
        assert_eq!(reverb_feedback(0.5), 0.625);
    }

    #[test]
    fn should_initialize_reverb_state() {
        let spec = ReverbSpec::new(0.5, 0.2, 0.3);
        let state = ReverbState::new(&spec);

        assert_eq!(state.left.combs.len(), 4);
        assert_eq!(state.left.allpasses.len(), 2);
        assert_eq!(state.right.combs.len(), 4);
        assert_eq!(state.right.allpasses.len(), 2);
        assert_eq!(state.wet, 0.3);

        assert_eq!(state.left.combs[0].feedback, 0.625);
        assert_eq!(state.left.combs[0].damp, 0.2);
    }

    #[test]
    fn should_sync_reverb_spec() {
        let spec1 = ReverbSpec::new(0.5, 0.2, 0.3);
        let mut state = ReverbState::new(&spec1);

        let spec2 = ReverbSpec::new(1.0, 0.8, 0.5);
        state.sync_spec(&spec2);

        assert_eq!(state.wet, 0.5);
        assert_eq!(state.left.combs[0].feedback, 0.9);
        assert_eq!(state.left.combs[0].damp, 0.8);
    }

    #[test]
    fn should_process_reverb_frame() {
        let spec = ReverbSpec::new(0.5, 0.2, 0.3);
        let mut state = ReverbState::new(&spec);

        let out = state.process_frame(1.0, -1.0);
        // initial delay lines are zero, but processing updates internal indices
        assert_eq!(out, (0.0, 0.0));
    }

    #[test]
    fn should_reset_reverb_state() {
        let spec = ReverbSpec::new(0.5, 0.2, 0.3);
        let mut state = ReverbState::new(&spec);

        let _ = state.process_frame(1.0, 1.0);
        state.reset();

        for comb in &state.left.combs {
            assert_eq!(comb.index, 0);
            assert_eq!(comb.filter_store, 0.0);
            assert!(comb.buffer.iter().all(|&x| x == 0.0));
        }

        for allpass in &state.left.allpasses {
            assert_eq!(allpass.index, 0);
            assert!(allpass.buffer.iter().all(|&x| x == 0.0));
        }
    }

    #[test]
    fn should_process_comb_filter() {
        let mut comb = CombState::new(4, 0.5, 0.2);

        // Frame 1
        let out1 = comb.process(1.0);
        assert_eq!(out1, 0.0);
        assert_eq!(comb.buffer[0], 1.0);
        assert_eq!(comb.index, 1);

        // Advance to loop point
        let _ = comb.process(0.0);
        let _ = comb.process(0.0);
        let _ = comb.process(0.0);

        // Frame 5 (feedback occurs)
        let out5 = comb.process(0.0);
        assert_eq!(out5, 1.0); // previous input comes out
        assert_eq!(comb.index, 1);
    }

    #[test]
    fn should_process_allpass_filter() {
        let mut allpass = AllpassState::new(2, 0.5);

        // Frame 1
        let out1 = allpass.process(1.0);
        assert_eq!(out1, -1.0); // 0.0 - 1.0
        assert_eq!(allpass.buffer[0], 1.0); // 0.0 + 0.5 * 1.0 -> wait, 0.0 * 0.5 + 1.0 = 1.0
        assert_eq!(allpass.index, 1);

        // Frame 2
        let out2 = allpass.process(0.0);
        assert_eq!(out2, 0.0);
        assert_eq!(allpass.buffer[1], 0.0);
        assert_eq!(allpass.index, 0);

        // Frame 3 (feedback occurs)
        let out3 = allpass.process(0.0);
        assert_eq!(out3, 1.0); // buffered 1.0 - 0.0 = 1.0
    }
}
