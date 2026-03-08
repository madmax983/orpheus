//! Audio engine for Orpheus.

/// Minimal engine handle placeholder used to link the workspace.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EngineHandle;

impl EngineHandle {
    /// Returns a no-op engine handle for the bootstrap smoke test.
    #[must_use]
    pub const fn stub() -> Self {
        Self
    }
}
