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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
    Pattern(Box<Self>),
    Sample,
    Note,
    Number,
    Duration,
    String,
    Function(Vec<Self>, Box<Self>),
    Var(TypeVarId),
    Unit,
}

impl Type {
    #[must_use]
    pub fn pattern(inner: Self) -> Self {
        Self::Pattern(Box::new(inner))
    }

    #[must_use]
    pub fn function(args: Vec<Self>, ret: Self) -> Self {
        Self::Function(args, Box::new(ret))
    }

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedModule {
    bindings: BTreeMap<String, Type>,
}

impl TypedModule {
    pub(crate) const fn new(bindings: BTreeMap<String, Type>) -> Self {
        Self { bindings }
    }

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
