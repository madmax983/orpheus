with open("crates/orpheus-lang/src/pedal.rs", "r") as f:
    content = f.read()

# Update ValidatedPedalBinding
content = content.replace("    /// Returns the variable name used for this binding.", "    /// Provides access to the immutable identifier that maps to the underlying node.")
content = content.replace("    /// Returns the pedal node that this binding references.", "    /// Exposes the resolved node graph that this identifier points to, allowing downward traversal.")

# Update TypeEnv::values (maybe that is okay, but let's check it)
# The reviewer didn't complain about TypeEnv::values text, only name() and node() in pedal.rs.

with open("crates/orpheus-lang/src/pedal.rs", "w") as f:
    f.write(content)
