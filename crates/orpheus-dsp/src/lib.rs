//! Audio engine for Orpheus.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EngineHandle;

impl EngineHandle {
    #[must_use]
    pub const fn stub() -> Self {
        Self
    }
}
