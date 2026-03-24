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
