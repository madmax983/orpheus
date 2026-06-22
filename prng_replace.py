import sys

def main():
    with open("crates/orpheus-lang/src/value.rs", "r") as f:
        content = f.read()

    old_code = """const fn deterministic_prng(site_salt: u64, cycle: i128) -> u64 {
    let [
        b0,
        b1,
        b2,
        b3,
        b4,
        b5,
        b6,
        b7,
        b8,
        b9,
        b10,
        b11,
        b12,
        b13,
        b14,
        b15,
    ] = cycle.to_le_bytes();
    let lower = u64::from_le_bytes([b0, b1, b2, b3, b4, b5, b6, b7]);
    let upper = u64::from_le_bytes([b8, b9, b10, b11, b12, b13, b14, b15]);"""

    new_code = """const fn deterministic_prng(site_salt: u64, cycle: i128) -> u64 {
    let lower = cycle as u64;
    let upper = (cycle >> 64) as u64;"""

    if old_code in content:
        content = content.replace(old_code, new_code)
        with open("crates/orpheus-lang/src/value.rs", "w") as f:
            f.write(content)
        print("Success")
    else:
        print("Failed to find old code block")

if __name__ == "__main__":
    main()
