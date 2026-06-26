fn cycle_fraction_to_frames(fraction: f64, frames_per_cycle: u64) -> Option<u32> {
    if !fraction.is_finite() || fraction <= 0.0 {
        return None;
    }

    let frames = (fraction * (frames_per_cycle as f64))
        .round()
        .clamp(1.0, f64::from(u32::MAX));
    Some(frames as u32)
}

fn main() {
    let delay_frames = cycle_fraction_to_frames(1e-10, 44100).unwrap();
    let buffer_len = delay_frames as usize;
    println!("buffer_len: {}", buffer_len);
}
