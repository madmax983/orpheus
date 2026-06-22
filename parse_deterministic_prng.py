import sys

def main():
    with open("crates/orpheus-lang/src/value.rs", "r") as f:
        lines = f.readlines()

    for idx, line in enumerate(lines):
        if "fn deterministic_prng" in line:
            start = idx
            break

    print("".join(lines[start:start+40]))

if __name__ == "__main__":
    main()
