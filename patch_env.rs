<<<<<<< SEARCH
    #[must_use]
    #[doc(hidden)]
    #[allow(clippy::too_many_lines)]
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
        env.insert("jux", jux_transform_scheme());
        env.insert("rev", unary_pattern_transform_scheme(alpha));
        env.insert("chaos", unary_pattern_transform_scheme(alpha));
        for name in [
            "gain", "hpf", "lpf", "cutoff", "res", "drive", "pw", "pan", "rate", "onset", "p1",
            "p2", "p3", "p4",
        ] {
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
=======
    #[must_use]
    #[doc(hidden)]
    pub fn with_builtins() -> Self {
        let mut env = Self {
            entries: BTreeMap::new(),
        };
        for name in ["bd", "sn", "cp", "hh", "saw", "pulse", "tri", "noise"] {
            env.insert(name, TypeScheme::monomorphic(Type::pattern(Type::Sample)));
        }

        Self::install_transform_builtins(&mut env);
        Self::install_plugin_and_scale_builtins(&mut env);
        Self::install_fx_and_utility_builtins(&mut env);

        env
    }

    fn install_transform_builtins(env: &mut Self) {
        let alpha = TypeVarId::new(0);
        install_cycle_alternation_builtins(env, alpha);
        for name in ["fast", "slow", "shift"] {
            env.insert(name, numeric_pattern_transform_scheme(alpha));
        }
        env.insert("every", every_transform_scheme(alpha));
        env.insert("when", when_transform_scheme(alpha));
        install_probabilistic_builtins(env, alpha);
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

        install_euclidean_and_counting_builtins(env, alpha);
    }

    fn install_plugin_and_scale_builtins(env: &mut Self) {
        let alpha = TypeVarId::new(0);
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
        install_plugin_builtins(env);

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
        env.insert("jux", jux_transform_scheme());
        env.insert("rev", unary_pattern_transform_scheme(alpha));
        env.insert("chaos", unary_pattern_transform_scheme(alpha));
    }

    fn install_fx_and_utility_builtins(env: &mut Self) {
        for name in [
            "gain", "hpf", "lpf", "cutoff", "res", "drive", "pw", "pan", "rate", "onset", "p1",
            "p2", "p3", "p4",
        ] {
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
    }
>>>>>>> REPLACE
