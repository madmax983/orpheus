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
}

fn main() {
    let mut allpass = AllpassState::new(2, 0.5);

    let out1 = allpass.process(1.0);
    println!("Frame 1 out: {}, buf: {}, index: {}", out1, allpass.buffer[0], allpass.index);

    let out2 = allpass.process(0.0);
    println!("Frame 2 out: {}, buf: {}, index: {}", out2, allpass.buffer[1], allpass.index);

    let out3 = allpass.process(0.0);
    println!("Frame 3 out: {}, buf: {}, index: {}", out3, allpass.buffer[0], allpass.index);
}
