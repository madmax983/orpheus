import os
from glob import glob

test_files = glob("crates/*/tests/*.rs")

for filepath in test_files:
    with open(filepath, 'r') as f:
        content = f.read()

    if not content.startswith('//!'):
        filename = os.path.basename(filepath)
        doc = f"//! Test suite for `{filename}` module functionality.\n//!\n//! Verifies correct execution of the `{filename}` tests.\n\n"
        with open(filepath, 'w') as f:
            f.write(doc + content)
        print(f"Added module documentation to {filepath}")
