//! The `types::env` module implements Hindley-Milner type environments.
//!
//! A type environment (`TypeEnv`) maps variable names to `TypeScheme`s, which allow
//! polymorphic functions (like `fast` or `rev`) to be instantiated with different concrete
//! types at different call sites.
//!
//! The environment is pre-populated with standard built-in functions via `TypeEnv::with_builtins()`.

use std::collections::BTreeMap;

use crate::types::{Type, TypeVarId};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeEnv {
    entries: BTreeMap<String, TypeScheme>,
}

impl TypeEnv {
    #[must_use]
    pub fn with_builtins() -> Self {
        let mut env = Self {
            entries: BTreeMap::new(),
        };
        env.insert("bd", TypeScheme::monomorphic(Type::pattern(Type::Sample)));
        env.insert("sn", TypeScheme::monomorphic(Type::pattern(Type::Sample)));
        env.insert("cp", TypeScheme::monomorphic(Type::pattern(Type::Sample)));
        env.insert("hh", TypeScheme::monomorphic(Type::pattern(Type::Sample)));

        let alpha = TypeVarId::new(0);
        for name in ["fast", "slow", "shift"] {
            env.insert(name, numeric_pattern_transform_scheme(alpha));
        }
        env.insert("every", every_transform_scheme(alpha));
        env.insert("when", when_transform_scheme(alpha));
        env.insert("sometimes", sometimes_transform_scheme(alpha));
        env.insert("within", within_transform_scheme(alpha));
        env.insert("mask", mask_scheme());
        env.insert("euclid", euclid_scheme());
        env.insert("degrees", degrees_scheme());
        env.insert("transpose", number_pattern_control_scheme());
        env.insert("jux", jux_transform_scheme());
        env.insert("rev", unary_pattern_transform_scheme(alpha));
        for name in ["gain", "hpf", "lpf", "pan", "pitch", "rate"] {
            env.insert(name, sample_control_scheme());
        }
        env.insert(
            "sample",
            TypeScheme::monomorphic(Type::curried(
                vec![Type::String],
                Type::pattern(Type::Sample),
            )),
        );
        env.insert(
            "slice",
            TypeScheme::monomorphic(Type::curried(
                vec![
                    Type::pattern(Type::Number),
                    Type::pattern(Type::Number),
                    Type::pattern(Type::Sample),
                ],
                Type::pattern(Type::Sample),
            )),
        );
        env.insert(
            "slice_idx",
            TypeScheme::monomorphic(Type::curried(
                vec![
                    Type::pattern(Type::Number),
                    Type::pattern(Type::Number),
                    Type::pattern(Type::Sample),
                ],
                Type::pattern(Type::Sample),
            )),
        );
        env.insert(
            "rand",
            TypeScheme::monomorphic(Type::function(vec![], Type::pattern(Type::Number))),
        );

        env
    }

    pub fn insert(&mut self, name: impl Into<String>, scheme: TypeScheme) {
        self.entries.insert(name.into(), scheme);
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&TypeScheme> {
        self.entries.get(name)
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &TypeScheme> {
        self.entries.values()
    }
}

fn sample_control_scheme() -> TypeScheme {
    TypeScheme::monomorphic(Type::curried(
        vec![Type::pattern(Type::Number), Type::pattern(Type::Sample)],
        Type::pattern(Type::Sample),
    ))
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

fn euclid_scheme() -> TypeScheme {
    TypeScheme::monomorphic(Type::curried(
        vec![Type::pattern(Type::Number), Type::pattern(Type::Number)],
        Type::pattern(Type::Number),
    ))
}

fn degrees_scheme() -> TypeScheme {
    TypeScheme::monomorphic(Type::curried(
        vec![Type::String, Type::pattern(Type::Number)],
        Type::pattern(Type::Number),
    ))
}

fn number_pattern_control_scheme() -> TypeScheme {
    TypeScheme::monomorphic(Type::curried(
        vec![Type::pattern(Type::Number), Type::pattern(Type::Number)],
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
}
