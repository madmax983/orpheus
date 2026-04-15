import re

# Add doc comments to export.rs
with open("crates/orpheus-lang/src/export.rs", "r") as f:
    content = f.read()

old_block = """    let mut separators = String::with_capacity(header.len());
    for (i, s) in header.split('|').enumerate() {"""
new_block = """    // ⚡ Bolt: Avoid intermediate `Vec` allocation and `join` overhead when building the table separator.
    let mut separators = String::with_capacity(header.len());
    for (i, s) in header.split('|').enumerate() {"""
content = content.replace(old_block, new_block)
with open("crates/orpheus-lang/src/export.rs", "w") as f:
    f.write(content)

# Add doc comments to eval.rs
with open("crates/orpheus-lang/src/eval.rs", "r") as f:
    content = f.read()

old_block = """    let (numerator, denominator) = if let Some((whole, fractional)) = digits.split_once('.') {
        let scale = checked_pow10(fractional.len())?;
        let mut combined = String::with_capacity(whole.len() + fractional.len());"""
new_block = """    let (numerator, denominator) = if let Some((whole, fractional)) = digits.split_once('.') {
        let scale = checked_pow10(fractional.len())?;
        // ⚡ Bolt: Avoid intermediate format string allocations by using String::with_capacity directly.
        let mut combined = String::with_capacity(whole.len() + fractional.len());"""
content = content.replace(old_block, new_block)
with open("crates/orpheus-lang/src/eval.rs", "w") as f:
    f.write(content)

# Add doc comments to stats.rs
with open("crates/orpheus-lang/src/stats.rs", "r") as f:
    content = f.read()

old_block = """    let unique_count = samples.len();
    let mut sample_list = String::new();
    for (i, sample) in samples.into_iter().enumerate() {"""
new_block = """    let unique_count = samples.len();
    // ⚡ Bolt: Avoid intermediate `Vec` allocation and `join` overhead by building the string sequentially.
    let mut sample_list = String::new();
    for (i, sample) in samples.into_iter().enumerate() {"""
content = content.replace(old_block, new_block)
with open("crates/orpheus-lang/src/stats.rs", "w") as f:
    f.write(content)
