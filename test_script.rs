use orpheus_pattern::Rational;

fn rational_to_frame_offset(value: &Rational, frames_per_cycle: u64) -> Option<u64> {
    if value.numerator() < 0 {
        return None;
    }
    let scaled = value
        .numerator()
        .checked_mul(i128::from(frames_per_cycle))?;
    let offset = scaled / value.denominator();
    u64::try_from(offset).ok()
}

fn main() {
    let r = Rational::new(1, 4).unwrap();
    println!("{:?}", rational_to_frame_offset(&r, 44100));
}
