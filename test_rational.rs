use orpheus_pattern::Rational;
fn main() {
    let r1 = Rational::new(1, i64::MAX).unwrap();
    let r2 = Rational::new(1, i64::MAX - 1).unwrap();
    let r3 = r1 + r2;
    println!("{:?}", r3);

    // Now we have a denominator of ~2^126. Add something coprime.
    let r4 = Rational::new(1, i64::MAX - 2).unwrap();
    let r5 = r3 + r4;
    println!("{:?}", r5);
}
