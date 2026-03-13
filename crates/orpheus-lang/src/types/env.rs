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
        let alpha_pattern = Type::pattern(Type::Var(alpha));
        env.insert(
            "fast",
            TypeScheme {
                vars: vec![alpha],
                ty: Type::curried(
                    vec![Type::pattern(Type::Number), alpha_pattern.clone()],
                    alpha_pattern.clone(),
                ),
            },
        );
        env.insert(
            "slow",
            TypeScheme {
                vars: vec![alpha],
                ty: Type::curried(
                    vec![Type::pattern(Type::Number), alpha_pattern.clone()],
                    alpha_pattern.clone(),
                ),
            },
        );
        env.insert(
            "shift",
            TypeScheme {
                vars: vec![alpha],
                ty: Type::curried(
                    vec![Type::pattern(Type::Number), alpha_pattern.clone()],
                    alpha_pattern.clone(),
                ),
            },
        );
        env.insert(
            "every",
            TypeScheme {
                vars: vec![alpha],
                ty: Type::curried(
                    vec![
                        Type::pattern(Type::Number),
                        Type::function(vec![alpha_pattern.clone()], alpha_pattern.clone()),
                        alpha_pattern.clone(),
                    ],
                    alpha_pattern.clone(),
                ),
            },
        );
        env.insert(
            "rev",
            TypeScheme {
                vars: vec![alpha],
                ty: Type::curried(vec![alpha_pattern.clone()], alpha_pattern),
            },
        );
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

        env
    }

    pub fn insert(&mut self, name: impl Into<String>, scheme: TypeScheme) {
        self.entries.insert(name.into(), scheme);
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&TypeScheme> {
        self.entries.get(name)
    }
}

fn sample_control_scheme() -> TypeScheme {
    TypeScheme::monomorphic(Type::curried(
        vec![Type::pattern(Type::Number), Type::pattern(Type::Sample)],
        Type::pattern(Type::Sample),
    ))
}
