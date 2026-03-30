import os

def fix_file(path):
    if not os.path.exists(path): return
    with open(path, "r") as f:
        content = f.read()

    content = content.replace("sample.frames().is_empty()", "sample.frames().is_empty()")
    content = content.replace("sample.frames.is_empty()", "sample.frames().is_empty()")
    content = content.replace("sample.frames()[", "sample.frames()[")

    with open(path, "w") as f:
        f.write(content)

fix_file("crates/orpheus-dsp/tests/sample.rs")

def fix_sample_rs(path):
    if not os.path.exists(path): return
    with open(path, "r") as f:
        content = f.read()

    content = content.replace("#[cfg(test)]", "")
    content = content.replace("pub(crate) fn frames(&self)", "pub fn frames(&self)")

    with open(path, "w") as f:
        f.write(content)

fix_sample_rs("crates/orpheus-dsp/src/sample.rs")
