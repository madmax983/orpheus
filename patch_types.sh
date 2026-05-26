#!/bin/bash
sed -i 's/pub(crate) struct TypeEnv/pub struct TypeEnv/g' crates/orpheus-lang/src/types/env.rs
sed -i 's/pub(crate) struct TypeScheme/pub struct TypeScheme/g' crates/orpheus-lang/src/types/env.rs
