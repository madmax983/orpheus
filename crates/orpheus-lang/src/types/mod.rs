//! The Hindley-Milner type system for the Orpheus language.
//!
//! This module defines the core type abstractions, environments, and inference engines
//! used to statically verify Orpheus programs before execution. The type system supports
//! polymorphism, type inference, and structural pattern matching.
//!
//! The entry points for type inference are `infer_module` and `infer_into_bindings`,
//! which evaluate AST sequences against a `TypeEnv` to produce a `TypedModule`.

#[allow(clippy::redundant_pub_crate)]
pub(crate) mod env;
mod infer;

use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};

pub use env::TypeEnv;
pub use env::TypeScheme;
pub use infer::infer_into_bindings;
pub use infer::infer_module;

/// A unique identifier representing a universally quantified type variable inside a `TypeScheme`.
///
/// # Examples
///
/// ```
/// use orpheus_lang::TypeVarId;
/// // The inner value is internal, so we don't instantiate it directly here.
/// ```
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TypeVarId(u32);

impl TypeVarId {
    const fn new(raw: u32) -> Self {
        Self(raw)
    }
}

/// Represents the fundamental semantic types within the Orpheus type system.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
    /// A repeating sequence of events in time. Patterns can contain any inner type,
    /// enabling structures like `Pattern<Sample>` or `Pattern<Number>`.
    Pattern(Box<Self>),
    /// A named audio file from a loaded sample bank (e.g., `"kick"`).
    Sample,
    /// A behavior-first pedal graph value.
    Pedal,
    /// A user-defined graph voice program (`voice { ... }`).
    Voice,
    /// A headless hosted plugin instrument.
    Plugin,
    /// A discrete musical pitch or frequency representation.
    Note,
    /// A generic numeric value, primarily used for DSP parameters like gain or filter cutoff.
    Number,
    /// A musical time interval (e.g., used to offset patterns or sequences).
    Duration,
    /// The direction an arpeggiator traverses a chord (e.g., "up", "down").
    ArpDirection,
    /// A collection of pitch classes that define a musical chord or scale.
    PitchClassSet,
    /// A microtonal tuning scale.
    Tuning,
    /// Textual data.
    String,
    /// A callable operation mapping arguments to a return value.
    /// Supports polymorphic behavior.
    Function(Vec<Self>, Box<Self>),
    /// A generic placeholder during type inference, representing an unknown
    /// or unbound type that will later unify with a concrete type.
    Var(TypeVarId),
    /// A type signifying no meaningful data. Usually represents side-effects
    /// or empty states.
    Unit,
}

impl Type {
    /// Constructs a `Pattern` type wrapping the given inner type.
    ///
    /// This is a convenience helper to avoid manually allocating `Box::new`.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_lang::Type;
    ///
    /// let pat_ty = Type::pattern(Type::Number);
    /// assert!(matches!(pat_ty, Type::Pattern(_)));
    /// ```
    #[must_use]
    pub fn pattern(inner: Self) -> Self {
        Self::Pattern(Box::new(inner))
    }

    /// Constructs a `Function` type with the given arguments and return type.
    ///
    /// This is a convenience helper to avoid manually allocating `Box::new`.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_lang::Type;
    ///
    /// let func_ty = Type::function(vec![Type::Number], Type::Pattern(Box::new(Type::Sample)));
    /// assert!(matches!(func_ty, Type::Function(_, _)));
    /// ```
    #[must_use]
    pub fn function(args: Vec<Self>, ret: Self) -> Self {
        Self::Function(args, Box::new(ret))
    }

    /// Constructs a sequence of curried `Function` types.
    ///
    /// Transforms `(A, B) -> C` into `A -> (B -> C)`. This is necessary for
    /// Hindley-Milner type inference which strictly evaluates unary functions.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_lang::Type;
    ///
    /// // Represents taking a Number, returning a function that takes a Sample, returning a Pattern<Sample>
    /// let curried_ty = Type::curried(vec![Type::Number, Type::Sample], Type::pattern(Type::Sample));
    /// assert!(matches!(curried_ty, Type::Function(_, _)));
    /// ```
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
            Self::Pedal => formatter.write_str("Pedal"),
            Self::Voice => formatter.write_str("Voice"),
            Self::Plugin => formatter.write_str("Plugin"),
            Self::Note => formatter.write_str("Note"),
            Self::Number => formatter.write_str("Number"),
            Self::Duration => formatter.write_str("Duration"),
            Self::ArpDirection => formatter.write_str("ArpDirection"),
            Self::PitchClassSet => formatter.write_str("PitchClassSet"),
            Self::Tuning => formatter.write_str("Tuning"),
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

/// A module resulting from successful type inference.
///
/// Contains the resolved monomorphic types for all top-level bindings defined
/// in the module.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, infer_module};
///
/// let module = infer_module("f = bd", ReplMode::Strict).unwrap();
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedModule {
    bindings: BTreeMap<String, Type>,
}

impl TypedModule {
    /// Inject raw bindings into a constructed environment.
    #[must_use]
    pub const fn new(bindings: BTreeMap<String, Type>) -> Self {
        Self { bindings }
    }

    /// Checks whether a specific variable name was inferred during type checking.
    ///
    /// # Examples
    ///
    /// ```
    /// // Using internal type constructor, so we test with inference output.
    /// use orpheus_lang::{infer_module, ReplMode, Type};
    ///
    /// let typed = infer_module("x = bd", ReplMode::Loose).unwrap();
    /// assert!(typed.contains_key("x"));
    /// assert!(!typed.contains_key("y"));
    /// ```
    #[must_use]
    pub fn contains_key(&self, name: &str) -> bool {
        self.bindings.contains_key(name)
    }

    /// Extracts the inferred Hindley-Milner type signature for a bound variable or function.
    ///
    /// Once an Orpheus expression is parsed and bound to a name in the environment, the type
    /// inference engine calculates its principal type. This method allows the REPL or TUI to
    /// display that type back to the user (e.g. telling them that `fast` is `Number -> Pattern -> Pattern`).
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_lang::{infer_module, ReplMode, Type};
    ///
    /// let typed = infer_module("x = bd", ReplMode::Loose).unwrap();
    /// assert_eq!(typed.type_of("x"), &Type::pattern(Type::Sample));
    /// ```
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
        assert_eq!(Type::Pedal.to_string(), "Pedal");
        assert_eq!(Type::Plugin.to_string(), "Plugin");
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
