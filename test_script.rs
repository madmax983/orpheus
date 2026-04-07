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
}

fn main() {
    let mut comb = CombState::new(4, 0.5, 0.2);

    let out1 = comb.process(1.0);
    println!("Frame 1 out: {}, buf: {}, index: {}", out1, comb.buffer[0], comb.index);

    // Frame 2
    comb.process(0.0);
    // Frame 3
    comb.process(0.0);
    // Frame 4
    comb.process(0.0);

    // Frame 5 (feedback occurs)
    let out5 = comb.process(0.0);
    println!("Frame 5 out: {}, buf: {}, index: {}", out5, comb.buffer[0], comb.index);
}
