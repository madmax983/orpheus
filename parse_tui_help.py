with open("crates/orpheus-lang/src/tui/mod.rs") as f:
    lines = f.readlines()
for i, line in enumerate(lines):
    if "fn help_overlay_body" in line:
        print(f"found at {i+1}")
