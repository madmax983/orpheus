mod env;
mod infer;

use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};

pub use infer::infer_into_bindings;
pub use infer::infer_module;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TypeVarId(u32);

impl TypeVarId {
    const fn new(raw: u32) -> Self {
        Self(raw)
    }
}

/// Represents the resolved type of an Orpheus expression or binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
    /// A pattern over time producing the given inner type.
    Pattern(Box<Self>),
    /// A discrete sample event.
    Sample,
    /// A melodic note event.
    Note,
    /// A generic numeric value.
    Number,
    /// A span of rational time.
    Duration,
    /// A string literal.
    String,
    /// A callable function with argument types and a return type.
    Function(Vec<Self>, Box<Self>),
    /// An unresolved type variable used during inference.
    Var(TypeVarId),
    /// The unit type (void).
    Unit,
}

impl Type {
    /// Helper to wrap an inner type in a `Pattern`.
    #[must_use]
    pub fn pattern(inner: Self) -> Self {
        Self::Pattern(Box::new(inner))
    }

    /// Helper to construct a `Function` type with the given arguments and return type.
    #[must_use]
    pub fn function(args: Vec<Self>, ret: Self) -> Self {
        Self::Function(args, Box::new(ret))
    }

    /// Helper to construct a curried `Function` type from arguments and return type.
    #[must_use]
    pub fn curried(args: Vec<Self>, ret: Self) -> Self {
        args.into_iter()
            .rev()
            .fold(ret, |ret, arg| Self::function(vec![arg], ret))
    }
}

impl Display for Type {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pattern(inner) => write!(formatter, "Pattern<{inner}>"),
            Self::Sample => formatter.write_str("Sample"),
            Self::Note => formatter.write_str("Note"),
            Self::Number => formatter.write_str("Number"),
            Self::Duration => formatter.write_str("Duration"),
            Self::String => formatter.write_str("String"),
            Self::Function(args, ret) => {
                formatter.write_str("Function(")?;
                for (index, arg) in args.iter().enumerate() {
                    if index > 0 {
                        formatter.write_str(", ")?;
                    }
                    write!(formatter, "{arg}")?;
                }
                write!(formatter, ") -> {ret}")
            }
            Self::Var(id) => write!(formatter, "t{}", id.0),
            Self::Unit => formatter.write_str("Unit"),
        }
    }
}

/// A successfully type-checked Orpheus module containing the inferred types for
/// all its bindings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedModule {
    bindings: BTreeMap<String, Type>,
}

impl TypedModule {
    pub(crate) const fn new(bindings: BTreeMap<String, Type>) -> Self {
        Self { bindings }
    }

    /// Returns `true` if the module contains an inferred binding with the given name.
    #[must_use]
    pub fn contains_key(&self, name: &str) -> bool {
        self.bindings.contains_key(name)
    }

    /// Returns the inferred type for a named binding.
    ///
    /// # Panics
    ///
    /// Panics if `name` does not exist in this typed module.
    #[must_use]
    pub fn type_of(&self, name: &str) -> &Type {
        self.bindings
            .get(name)
            .unwrap_or_else(|| panic!("no inferred binding named `{name}`"))
    }
}
