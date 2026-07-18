//! The `types::env` module implements Hindley-Milner type environments.
//!
//! A type environment (`TypeEnv`) maps variable names to `TypeScheme`s, which allow
//! polymorphic functions (like `fast` or `rev`) to be instantiated with different concrete
//! types at different call sites.
//!
//! The environment is pre-populated with standard built-in functions via `TypeEnv::with_builtins()`.

use std::collections::BTreeMap;

use crate::types::{Type, TypeVarId};

/// A polymorphic type scheme containing universally quantified type variables.
///
/// This allows functions like `fast` to operate on `Pattern<t>` regardless of
/// whether `t` is a `Sample` or a `Number`. During type inference, the scheme
/// is instantiated to produce a concrete type for each specific usage.
///
/// # Examples
///
/// ```
/// use orpheus_lang::Type;
/// use orpheus_lang::TypeScheme;
///
/// let scheme = TypeScheme::monomorphic(Type::Sample);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeScheme {
    pub vars: Vec<TypeVarId>,
    pub ty: Type,
}

impl TypeScheme {
    #[must_use]
    pub const fn monomorphic(ty: Type) -> Self {
        Self {
            vars: Vec::new(),
            ty,
        }
    }
}

/// A mapping from variable names to their corresponding `TypeScheme`s.
///
/// Stores both the predefined built-in primitives and any user-defined
/// variables created during a session.
///
/// # Examples
///
/// ```
/// use orpheus_lang::TypeEnv;
///
/// let env = TypeEnv::with_builtins();
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeEnv {
    entries: BTreeMap<String, TypeScheme>,
}

impl TypeEnv {
    /// Creates a new typing environment pre-populated with Orpheus built-ins.
    ///
    /// This includes fundamental sample identifiers (`bd`, `sn`), signal oscillators
    /// (`saw`, `tri`), and polymorphic temporal transforms (`fast`, `every`, `when`).
    ///
    #[must_use]
    #[doc(hidden)]
    pub fn with_builtins() -> Self {
        let mut env = Self {
            entries: BTreeMap::new(),
        };
        for name in ["bd", "sn", "cp", "hh", "saw", "pulse", "tri", "noise"] {
            env.insert(name, TypeScheme::monomorphic(Type::pattern(Type::Sample)));
        }

        let alpha = TypeVarId::new(0);
        install_cycle_alternation_builtins(&mut env, alpha);
        for name in ["fast", "slow", "shift"] {
            env.insert(name, numeric_pattern_transform_scheme(alpha));
        }
        env.insert("every", every_transform_scheme(alpha));
        env.insert("when", when_transform_scheme(alpha));
        install_probabilistic_builtins(&mut env, alpha);
        env.insert("within", within_transform_scheme(alpha));
        env.insert("mask", mask_scheme());
        env.insert("strum", unary_number_pattern_scheme());
        env.insert("roll", numeric_pattern_transform_scheme(alpha));
        env.insert("arp", arp_scheme());
        env.insert("up", TypeScheme::monomorphic(Type::ArpDirection));
        env.insert("down", TypeScheme::monomorphic(Type::ArpDirection));

        for name in ["invert", "drop", "chord"] {
            env.insert(name, number_pattern_control_scheme());
        }
        // `transpose` and `pitch` are the interchangeable semitone-shift
        // pair: `Pattern<Number> -> Pattern<a> -> Pattern<a>` over number
        // and sample/voice patterns alike (reference-song gap-fix 3 of 3).
        for name in ["transpose", "pitch"] {
            env.insert(name, numeric_pattern_transform_scheme(alpha));
        }

        install_euclidean_and_counting_builtins(&mut env, alpha);
        env.insert(
            "pitch_class_set",
            TypeScheme::monomorphic(Type::curried(
                vec![Type::pattern(Type::Number)],
                Type::PitchClassSet,
            )),
        );
        env.insert("degrees", degrees_scheme());

        env.insert(
            "tuning",
            TypeScheme::monomorphic(Type::function(
                vec![Type::pattern(Type::Number)],
                Type::Tuning,
            )),
        );
        install_plugin_builtins(&mut env);

        install_scale_builtins(&mut env);
        env.insert("jux", jux_transform_scheme());
        env.insert("rev", unary_pattern_transform_scheme(alpha));
        env.insert("chaos", unary_pattern_transform_scheme(alpha));
        install_sample_control_builtins(&mut env);
        env.insert(
            "sample",
            TypeScheme::monomorphic(Type::curried(
                vec![Type::String],
                Type::pattern(Type::Sample),
            )),
        );
        env.insert(
            "through",
            TypeScheme::monomorphic(Type::curried(
                vec![Type::Pedal, Type::pattern(Type::Sample)],
                Type::pattern(Type::Sample),
            )),
        );

        for name in ["slice", "slice_idx"] {
            env.insert(
                name,
                TypeScheme::monomorphic(Type::curried(
                    vec![
                        Type::pattern(Type::Number),
                        Type::pattern(Type::Number),
                        Type::pattern(Type::Sample),
                    ],
                    Type::pattern(Type::Sample),
                )),
            );
        }

        env.insert(
            "rand",
            TypeScheme::monomorphic(Type::function(vec![], Type::pattern(Type::Number))),
        );
        for name in ["cc", "midi_cc"] {
            env.insert(
                name,
                TypeScheme::monomorphic(Type::curried(
                    vec![Type::pattern(Type::Number)],
                    Type::pattern(Type::Number),
                )),
            );
        }

        env
    }

    /// Inserts a new variable mapping into the type environment.
    ///
    /// This makes the variable available for subsequent type inferences in the same environment.
    ///
    #[doc(hidden)]
    pub fn insert(&mut self, name: impl Into<String>, scheme: TypeScheme) {
        self.entries.insert(name.into(), scheme);
    }

    /// Looks up a variable's type scheme in the environment.
    ///
    /// Returns `Some(&TypeScheme)` if the name exists, which can then be instantiated
    /// to yield a concrete `Type` during inference. Returns `None` if the name is unbound.
    ///
    #[must_use]
    #[doc(hidden)]
    pub fn get(&self, name: &str) -> Option<&TypeScheme> {
        self.entries.get(name)
    }

    pub fn values(&self) -> impl Iterator<Item = &TypeScheme> {
        self.entries.values()
    }
}

fn install_scale_builtins(env: &mut TypeEnv) {
    for name in [
        "ionian",
        "dorian",
        "phrygian",
        "mixolydian",
        "aeolian",
        "minor_pentatonic",
    ] {
        env.insert(name, TypeScheme::monomorphic(Type::PitchClassSet));
    }
}

fn install_sample_control_builtins(env: &mut TypeEnv) {
    for name in [
        "gain", "hpf", "lpf", "cutoff", "res", "drive", "pw", "pan", "rate", "onset", "p1", "p2",
        "p3", "p4",
    ] {
        env.insert(name, sample_control_scheme());
    }
}

fn sample_control_scheme() -> TypeScheme {
    TypeScheme::monomorphic(Type::curried(
        vec![Type::pattern(Type::Number), Type::pattern(Type::Sample)],
        Type::pattern(Type::Sample),
    ))
}

fn install_plugin_builtins(env: &mut TypeEnv) {
    for name in ["vst", "au"] {
        env.insert(
            name,
            TypeScheme::monomorphic(Type::curried(vec![Type::String], Type::Plugin)),
        );
    }
    env.insert(
        "notes",
        TypeScheme::monomorphic(Type::curried(
            vec![Type::pattern(Type::Number), Type::Plugin],
            Type::Plugin,
        )),
    );
    for name in ["p", "param"] {
        env.insert(
            name,
            TypeScheme::monomorphic(Type::curried(
                vec![Type::String, Type::pattern(Type::Number), Type::Plugin],
                Type::Plugin,
            )),
        );
    }
}

fn install_probabilistic_builtins(env: &mut TypeEnv, alpha: TypeVarId) {
    env.insert("sometimes", sometimes_transform_scheme(alpha));
    env.insert("degrade", unary_pattern_transform_scheme(alpha));
    env.insert("degrade_by", numeric_pattern_transform_scheme(alpha));
    // `sometimes_by` shares `every`'s shape:
    // Pattern<Number> -> (Pattern<a> -> Pattern<a>) -> Pattern<a> -> Pattern<a>.
    env.insert("sometimes_by", every_transform_scheme(alpha));
    for name in ["often", "rarely", "almost_always", "almost_never"] {
        env.insert(name, sometimes_transform_scheme(alpha));
    }
    // `segment` samples any pattern kind; `range`/`choose`/`wchoose`/`irand`
    // operate on numbers. `choose`/`wchoose` are variadic at runtime; calls
    // beyond the base arity are special-cased during inference.
    env.insert("segment", numeric_pattern_transform_scheme(alpha));
    env.insert("range", range_scheme());
    env.insert("choose", choose_scheme());
    env.insert("wchoose", wchoose_scheme());
    env.insert("irand", unary_number_pattern_scheme());
}

fn range_scheme() -> TypeScheme {
    TypeScheme::monomorphic(Type::curried(
        vec![
            Type::pattern(Type::Number),
            Type::pattern(Type::Number),
            Type::pattern(Type::Number),
        ],
        Type::pattern(Type::Number),
    ))
}

/// The two-value base scheme for `choose`:
/// `Pattern<Number> -> Pattern<Number> -> Pattern<Number>`.
///
/// `choose` is variadic at runtime; calls with more than two arguments are
/// special-cased during inference.
pub fn choose_scheme() -> TypeScheme {
    number_pattern_control_scheme()
}

/// The two-pair base scheme for `wchoose(v1, w1, v2, w2)`:
/// four `Pattern<Number>` arguments returning `Pattern<Number>`.
///
/// `wchoose` is variadic at runtime; calls with more than four arguments are
/// special-cased during inference.
pub fn wchoose_scheme() -> TypeScheme {
    TypeScheme::monomorphic(Type::curried(
        vec![Type::pattern(Type::Number); 4],
        Type::pattern(Type::Number),
    ))
}

fn install_cycle_alternation_builtins(env: &mut TypeEnv, alpha: TypeVarId) {
    // `pchoose` shares `randcat`'s shape (same-kind patterns in, one out)
    // and `wpchoose` shares `wrandcat`'s interleaved pattern/weight shape;
    // both draw per cycle slot instead of per cycle at runtime.
    for name in ["cat", "slowcat", "append", "randcat", "pchoose"] {
        env.insert(name, pattern_concat_scheme(alpha));
    }
    env.insert("wrandcat", wrandcat_scheme(alpha));
    env.insert("wpchoose", wrandcat_scheme(alpha));
    env.insert("markov", markov_scheme(alpha));
    for name in ["iter", "iter_back", "rot", "shuffle", "scramble"] {
        env.insert(name, numeric_pattern_transform_scheme(alpha));
    }
    // `off`, `chunk`, and `chunk_back` share `every`'s shape:
    // Pattern<Number> -> (Pattern<a> -> Pattern<a>) -> Pattern<a> -> Pattern<a>.
    for name in ["off", "chunk", "chunk_back"] {
        env.insert(name, every_transform_scheme(alpha));
    }
}

/// The two-pair base scheme for `wrandcat(p1, w1, p2, w2)`:
/// `Pattern<a> -> Pattern<Number> -> Pattern<a> -> Pattern<Number> -> Pattern<a>`.
///
/// `wrandcat` is variadic at runtime; calls with more than four arguments are
/// special-cased during inference.
pub fn wrandcat_scheme(alpha: TypeVarId) -> TypeScheme {
    let alpha_pattern = Type::pattern(Type::Var(alpha));
    TypeScheme {
        vars: vec![alpha],
        ty: Type::curried(
            vec![
                alpha_pattern.clone(),
                Type::pattern(Type::Number),
                alpha_pattern.clone(),
                Type::pattern(Type::Number),
            ],
            alpha_pattern,
        ),
    }
}

/// The two-state base scheme for `markov(s0, w00, w01, s1, w10, w11)`:
/// each state pattern is followed by its two `Pattern<Number>` transition
/// weights, returning the shared state pattern type.
///
/// `markov` is variadic at runtime; calls with more than six arguments are
/// special-cased during inference.
pub fn markov_scheme(alpha: TypeVarId) -> TypeScheme {
    let alpha_pattern = Type::pattern(Type::Var(alpha));
    let number_pattern = Type::pattern(Type::Number);
    TypeScheme {
        vars: vec![alpha],
        ty: Type::curried(
            vec![
                alpha_pattern.clone(),
                number_pattern.clone(),
                number_pattern.clone(),
                alpha_pattern.clone(),
                number_pattern.clone(),
                number_pattern,
            ],
            alpha_pattern,
        ),
    }
}

/// The two-argument scheme shared by `cat`, `slowcat`, and `append`:
/// `Pattern<a> -> Pattern<a> -> Pattern<a>`.
///
/// `cat`/`slowcat` are variadic at runtime; calls with more than two
/// arguments are special-cased during inference.
pub fn pattern_concat_scheme(alpha: TypeVarId) -> TypeScheme {
    let alpha_pattern = Type::pattern(Type::Var(alpha));
    TypeScheme {
        vars: vec![alpha],
        ty: Type::curried(
            vec![alpha_pattern.clone(), alpha_pattern.clone()],
            alpha_pattern,
        ),
    }
}

fn numeric_pattern_transform_scheme(alpha: TypeVarId) -> TypeScheme {
    let alpha_pattern = Type::pattern(Type::Var(alpha));
    TypeScheme {
        vars: vec![alpha],
        ty: Type::curried(
            vec![Type::pattern(Type::Number), alpha_pattern.clone()],
            alpha_pattern,
        ),
    }
}

fn every_transform_scheme(alpha: TypeVarId) -> TypeScheme {
    let alpha_pattern = Type::pattern(Type::Var(alpha));
    TypeScheme {
        vars: vec![alpha],
        ty: Type::curried(
            vec![
                Type::pattern(Type::Number),
                Type::function(vec![alpha_pattern.clone()], alpha_pattern.clone()),
                alpha_pattern.clone(),
            ],
            alpha_pattern,
        ),
    }
}

fn when_transform_scheme(alpha: TypeVarId) -> TypeScheme {
    let alpha_pattern = Type::pattern(Type::Var(alpha));
    TypeScheme {
        vars: vec![alpha],
        ty: Type::curried(
            vec![
                Type::pattern(Type::Number),
                Type::pattern(Type::Number),
                Type::function(vec![alpha_pattern.clone()], alpha_pattern.clone()),
                alpha_pattern.clone(),
            ],
            alpha_pattern,
        ),
    }
}

fn jux_transform_scheme() -> TypeScheme {
    let sample_pattern = Type::pattern(Type::Sample);
    TypeScheme::monomorphic(Type::curried(
        vec![
            Type::function(vec![sample_pattern.clone()], sample_pattern.clone()),
            sample_pattern.clone(),
        ],
        sample_pattern,
    ))
}

fn sometimes_transform_scheme(alpha: TypeVarId) -> TypeScheme {
    let alpha_pattern = Type::pattern(Type::Var(alpha));
    TypeScheme {
        vars: vec![alpha],
        ty: Type::curried(
            vec![
                Type::function(vec![alpha_pattern.clone()], alpha_pattern.clone()),
                alpha_pattern.clone(),
            ],
            alpha_pattern,
        ),
    }
}

fn within_transform_scheme(alpha: TypeVarId) -> TypeScheme {
    let alpha_pattern = Type::pattern(Type::Var(alpha));
    TypeScheme {
        vars: vec![alpha],
        ty: Type::curried(
            vec![
                Type::pattern(Type::Number),
                Type::pattern(Type::Number),
                Type::function(vec![alpha_pattern.clone()], alpha_pattern.clone()),
                alpha_pattern.clone(),
            ],
            alpha_pattern,
        ),
    }
}

fn mask_scheme() -> TypeScheme {
    let gate = TypeVarId::new(0);
    let pattern = TypeVarId::new(1);
    TypeScheme {
        vars: vec![gate, pattern],
        ty: Type::curried(
            vec![
                Type::pattern(Type::Var(gate)),
                Type::pattern(Type::Var(pattern)),
            ],
            Type::pattern(Type::Var(pattern)),
        ),
    }
}

fn install_euclidean_and_counting_builtins(env: &mut TypeEnv, alpha: TypeVarId) {
    // `whenmod` shares `when`'s shape (period, threshold, transform, pattern).
    env.insert("whenmod", when_transform_scheme(alpha));
    env.insert("euclid", euclid_scheme());
    // `euclid_inv` mirrors `euclid`; both are variadic at runtime (an
    // optional third rotation argument) and special-cased during inference.
    env.insert("euclid_inv", euclid_scheme());
    env.insert("euclid_full", euclid_full_scheme(alpha));
    // `run` and `scan` build counting number patterns from a step count.
    env.insert("run", unary_number_pattern_scheme());
    env.insert("scan", unary_number_pattern_scheme());
}

/// The two-argument base scheme shared by `euclid` and `euclid_inv`:
/// `Pattern<Number> -> Pattern<Number> -> Pattern<Number>`.
///
/// Both accept an optional third rotation argument at runtime; three-argument
/// calls are special-cased during inference.
pub fn euclid_scheme() -> TypeScheme {
    TypeScheme::monomorphic(Type::curried(
        vec![Type::pattern(Type::Number), Type::pattern(Type::Number)],
        Type::pattern(Type::Number),
    ))
}

/// The four-argument base scheme for `euclid_full(pulses, steps, hits, rests)`:
/// `Pattern<Number> -> Pattern<Number> -> Pattern<a> -> Pattern<a> -> Pattern<a>`.
///
/// `euclid_full` accepts an optional rotation as a third argument at runtime
/// (`euclid_full(pulses, steps, rotation, hits, rests)`); five-argument calls
/// are special-cased during inference.
pub fn euclid_full_scheme(alpha: TypeVarId) -> TypeScheme {
    let alpha_pattern = Type::pattern(Type::Var(alpha));
    TypeScheme {
        vars: vec![alpha],
        ty: Type::curried(
            vec![
                Type::pattern(Type::Number),
                Type::pattern(Type::Number),
                alpha_pattern.clone(),
                alpha_pattern.clone(),
            ],
            alpha_pattern,
        ),
    }
}

fn arp_scheme() -> TypeScheme {
    TypeScheme::monomorphic(Type::curried(
        vec![
            Type::pattern(Type::Number),
            Type::ArpDirection,
            Type::pattern(Type::Number),
        ],
        Type::pattern(Type::Number),
    ))
}

fn degrees_scheme() -> TypeScheme {
    TypeScheme::monomorphic(Type::curried(
        vec![Type::PitchClassSet, Type::pattern(Type::Number)],
        Type::pattern(Type::Number),
    ))
}

fn number_pattern_control_scheme() -> TypeScheme {
    TypeScheme::monomorphic(Type::curried(
        vec![Type::pattern(Type::Number), Type::pattern(Type::Number)],
        Type::pattern(Type::Number),
    ))
}

fn unary_number_pattern_scheme() -> TypeScheme {
    TypeScheme::monomorphic(Type::curried(
        vec![Type::pattern(Type::Number)],
        Type::pattern(Type::Number),
    ))
}

fn unary_pattern_transform_scheme(alpha: TypeVarId) -> TypeScheme {
    let alpha_pattern = Type::pattern(Type::Var(alpha));
    TypeScheme {
        vars: vec![alpha],
        ty: Type::curried(vec![alpha_pattern.clone()], alpha_pattern),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_scheme_monomorphic_creates_empty_vars() {
        let scheme = TypeScheme::monomorphic(Type::Number);
        assert!(scheme.vars.is_empty());
        assert_eq!(scheme.ty, Type::Number);
    }

    #[test]
    fn type_env_builtins_contains_expected_types() {
        let env = TypeEnv::with_builtins();

        // Check monomorphic primitives.
        let bd_scheme = env.get("bd").expect("missing bd builtin");
        assert!(bd_scheme.vars.is_empty());
        assert_eq!(bd_scheme.ty, Type::pattern(Type::Sample));

        // Check rand function.
        let rand_scheme = env.get("rand").expect("missing rand builtin");
        assert!(rand_scheme.vars.is_empty());
        assert_eq!(
            rand_scheme.ty,
            Type::function(vec![], Type::pattern(Type::Number))
        );

        // Check polymorphic transforms.
        let fast_scheme = env.get("fast").expect("missing fast builtin");
        assert_eq!(fast_scheme.vars.len(), 1);
        let alpha = fast_scheme.vars[0];
        assert_eq!(
            fast_scheme.ty,
            Type::curried(
                vec![Type::pattern(Type::Number), Type::pattern(Type::Var(alpha))],
                Type::pattern(Type::Var(alpha))
            )
        );

        // Check sample controls.
        let gain_scheme = env.get("gain").expect("missing gain builtin");
        assert!(gain_scheme.vars.is_empty());
        assert_eq!(
            gain_scheme.ty,
            Type::curried(
                vec![Type::pattern(Type::Number), Type::pattern(Type::Sample)],
                Type::pattern(Type::Sample)
            )
        );
    }

    #[test]
    fn type_env_insert_and_get() {
        let mut env = TypeEnv::with_builtins();
        assert!(env.get("my_custom_var").is_none());

        env.insert("my_custom_var", TypeScheme::monomorphic(Type::Duration));

        let scheme = env.get("my_custom_var").unwrap();
        assert!(scheme.vars.is_empty());
        assert_eq!(scheme.ty, Type::Duration);
    }

    #[test]
    fn type_env_values_iterates_all_items() {
        let mut env = TypeEnv::with_builtins();
        let initial_count = env.values().count();
        assert!(initial_count > 0, "Builtin environment should not be empty");

        env.insert("test_val", TypeScheme::monomorphic(Type::Number));
        assert_eq!(env.values().count(), initial_count + 1);
    }
}
