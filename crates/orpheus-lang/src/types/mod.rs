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
    ArpDirection,
    PitchClassSet,
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
            Self::ArpDirection => formatter.write_str("ArpDirection"),
            Self::PitchClassSet => formatter.write_str("PitchClassSet"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_display_formats_correctly() {
        assert_eq!(Type::Sample.to_string(), "Sample");
        assert_eq!(Type::Note.to_string(), "Note");
        assert_eq!(Type::Number.to_string(), "Number");
        assert_eq!(Type::Duration.to_string(), "Duration");
        assert_eq!(Type::ArpDirection.to_string(), "ArpDirection");
        assert_eq!(Type::PitchClassSet.to_string(), "PitchClassSet");
        assert_eq!(Type::String.to_string(), "String");
        assert_eq!(Type::Unit.to_string(), "Unit");

        assert_eq!(Type::Var(TypeVarId::new(42)).to_string(), "t42");
        assert_eq!(Type::pattern(Type::Sample).to_string(), "Pattern<Sample>");
        assert_eq!(
            Type::function(
                vec![Type::Sample, Type::Number],
                Type::Pattern(Box::new(Type::Sample))
            )
            .to_string(),
            "Function(Sample, Number) -> Pattern<Sample>"
        );
        assert_eq!(
            Type::function(vec![], Type::Unit).to_string(),
            "Function() -> Unit"
        );
    }

    #[test]
    fn typed_module_contains_key_and_type_of_work() {
        let mut bindings = BTreeMap::new();
        bindings.insert("x".to_string(), Type::Number);
        let module = TypedModule::new(bindings);

        assert!(module.contains_key("x"));
        assert!(!module.contains_key("y"));
        assert_eq!(module.type_of("x"), &Type::Number);
    }

    #[test]
    #[should_panic(expected = "no inferred binding named `y`")]
    fn typed_module_type_of_panics_on_missing_key() {
        let module = TypedModule::new(BTreeMap::new());
        let _ = module.type_of("y");
    }

    #[test]
    fn type_constructors_build_expected_structures() {
        assert_eq!(
            Type::pattern(Type::Number),
            Type::Pattern(Box::new(Type::Number))
        );

        assert_eq!(
            Type::function(vec![Type::Sample], Type::Note),
            Type::Function(vec![Type::Sample], Box::new(Type::Note))
        );

        assert_eq!(
            Type::curried(vec![Type::Sample, Type::Number], Type::Note),
            Type::Function(
                vec![Type::Sample],
                Box::new(Type::Function(vec![Type::Number], Box::new(Type::Note)))
            )
        );

        assert_eq!(Type::curried(vec![], Type::Sample), Type::Sample);
    }
}
