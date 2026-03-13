#!/bin/bash

# Remove EvalError definition from eval.rs
sed -i '/\/\/\/ Runtime evaluation error for bootstrap Orpheus modules./, /impl Error for EvalError {}/d' crates/orpheus-lang/src/eval.rs
sed -i 's/use crate::diagnostics::ParseError;/use crate::diagnostics::{ParseError, EvalError};/' crates/orpheus-lang/src/eval.rs

# Add EvalError definition to diagnostics.rs
cat << 'INNER_EOF' >> crates/orpheus-lang/src/diagnostics.rs

/// Runtime evaluation error for bootstrap Orpheus modules.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvalError {
    message: Box<str>,
}

impl EvalError {
    pub(crate) fn new(message: impl Into<Box<str>>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for EvalError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for EvalError {}
INNER_EOF

# Update exports in lib.rs
sed -i 's/pub use diagnostics::{LoadError, ParseError, TypeError};/pub use diagnostics::{LoadError, ParseError, TypeError, EvalError};/' crates/orpheus-lang/src/lib.rs

sed -i 's/pub use eval::{/pub use eval::{/' crates/orpheus-lang/src/lib.rs
sed -i 's/    EvalError, RenderError, eval_module, render_sample_pattern_to_file,/    RenderError, eval_module, render_sample_pattern_to_file,/' crates/orpheus-lang/src/lib.rs

# Update imports in value.rs and builtins.rs
sed -i 's/use crate::eval::EvalError;/use crate::diagnostics::EvalError;/' crates/orpheus-lang/src/value.rs
sed -i 's/use crate::eval::{EvalError, f64_to_rational};/use crate::eval::f64_to_rational;\nuse crate::diagnostics::EvalError;/' crates/orpheus-lang/src/builtins.rs
