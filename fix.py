import re

with open("crates/orpheus-lang/src/eval.rs", "r") as f:
    content = f.read()

# Replace the specific block in eval.rs
old_block = """    let (numerator, denominator) = if let Some((whole, fractional)) = digits.split_once('.') {
        let scale = checked_pow10(fractional.len())?;
        let combined = format!("{whole}{fractional}");
        let numerator = combined
            .parse::<i128>()
            .map_err(|_| EvalError::new(format!("{context} exceeded the supported range")))?;
        (numerator, scale)"""

new_block = """    let (numerator, denominator) = if let Some((whole, fractional)) = digits.split_once('.') {
        let scale = checked_pow10(fractional.len())?;
        let mut combined = String::with_capacity(whole.len() + fractional.len());
        combined.push_str(whole);
        combined.push_str(fractional);
        let numerator = combined
            .parse::<i128>()
            .map_err(|_| EvalError::new(format!("{context} exceeded the supported range")))?;
        (numerator, scale)"""

content = content.replace(old_block, new_block)

with open("crates/orpheus-lang/src/eval.rs", "w") as f:
    f.write(content)
