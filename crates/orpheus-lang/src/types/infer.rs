//! Hindley-Milner type inference for the Orpheus language.
//!
//! This module provides the [`infer_module`] and [`infer_into_bindings`] functions,
//! which analyze parsed Abstract Syntax Trees ([`crate::ast::Module`]) and assign
//! concrete types to all top-level bindings.
//!
//! # Loose vs Strict Mode
//! The inference engine respects the active [`ReplMode`].
//! - In **Strict** mode, types must unify exactly, catching logic errors early in `.ode` files.
//! - In **Loose** mode (typical for the REPL), the engine permits implicit coercions
//!   (e.g., automatically lifting a single `Number` into a `Pattern<Number>`) to allow
//!   for rapid live-coding iteration without excessive ceremony.

use std::collections::{BTreeMap, BTreeSet};

use crate::ReplMode;
use crate::ast::{Expr, Module, Stmt, binding_expr_self_references};
use crate::diagnostics::{ParseError, TypeError};
use crate::parser::parse_module;
use crate::pitch::parse_named_pitch_literal;
use crate::types::env::{TypeEnv, TypeScheme};
use crate::types::{Type, TypeVarId, TypedModule};

/// Infers the types of top-level bindings in an Orpheus module.
///
/// # Errors
///
/// Returns [`TypeError`] when parsing fails or when type inference encounters
/// an unresolved name or incompatible types.
pub fn infer_module(source: &str, mode: ReplMode) -> Result<TypedModule, TypeError> {
    let parsed = parse_module(source).map_err(TypeError::from)?;
    Inferencer::new(mode).infer_module(&parsed)
}

/// Infers one source snippet against an existing binding environment.
///
/// # Errors
///
/// Returns [`TypeError`] when parsing fails or the new snippet does not type
/// check against the existing bindings.
pub fn infer_into_bindings(
    source: &str,
    mode: ReplMode,
    bindings: &mut BTreeMap<String, Type>,
) -> Result<Option<(String, Type)>, TypeError> {
    let parsed = parse_module(source).map_err(TypeError::from)?;
    // ⚡ Bolt: Use `std::mem::take` instead of `bindings.clone()` to move the BTreeMap into the inferencer.
    // This avoids a full heap allocation and deep copy of the environment on every inference pass.
    let mut inferencer = Inferencer::with_bindings(mode, std::mem::take(bindings));
    let result = inferencer.infer_statements(&parsed.statements);
    *bindings = inferencer.user_bindings;
    result
}

impl From<ParseError> for TypeError {
    fn from(error: ParseError) -> Self {
        Self::new(error.to_string())
    }
}

struct Inferencer {
    mode: ReplMode,
    env: TypeEnv,
    user_bindings: BTreeMap<String, Type>,
    substitutions: BTreeMap<TypeVarId, Type>,
    next_var: u32,
}

impl Inferencer {
    fn new(mode: ReplMode) -> Self {
        Self::with_bindings(mode, BTreeMap::new())
    }

    fn with_bindings(mode: ReplMode, bindings: BTreeMap<String, Type>) -> Self {
        let mut env = TypeEnv::with_builtins();
        for (name, ty) in &bindings {
            env.insert(name.clone(), TypeScheme::monomorphic(ty.clone()));
        }

        Self {
            mode,
            env,
            user_bindings: bindings,
            substitutions: BTreeMap::new(),
            next_var: 1,
        }
    }

    fn infer_module(mut self, module: &Module) -> Result<TypedModule, TypeError> {
        self.infer_statements(&module.statements)?;
        Ok(TypedModule::new(self.user_bindings))
    }

    fn infer_statements(
        &mut self,
        statements: &[Stmt],
    ) -> Result<Option<(String, Type)>, TypeError> {
        let mut last_binding = None;

        for statement in statements {
            match statement {
                Stmt::Binding {
                    name, params, expr, ..
                } => {
                    let inferred = self.infer_binding(name, params, expr)?;
                    let ty = self.resolve(inferred);
                    self.env.insert(name.clone(), self.generalize(ty.clone()));
                    self.user_bindings.insert(name.clone(), ty.clone());
                    last_binding = Some((name.clone(), ty));
                }
            }
        }

        Ok(last_binding)
    }

    fn infer_binding(
        &mut self,
        name: &str,
        params: &[String],
        expr: &Expr,
    ) -> Result<Type, TypeError> {
        if params.is_empty() {
            return self.infer_expr(expr);
        }

        if binding_expr_self_references(name, params, expr) {
            return Err(TypeError::new(format!(
                "parameterized binding `{name}` cannot contain a self-reference in v1"
            )));
        }

        let saved_env = self.env.clone();
        let result = (|| {
            let mut param_types = Vec::with_capacity(params.len());
            for param in params {
                let ty = self.fresh_var_type();
                self.env
                    .insert(param.clone(), TypeScheme::monomorphic(ty.clone()));
                param_types.push(ty);
            }

            let body_ty = self.infer_expr(expr)?;
            Ok(Type::curried(
                param_types
                    .into_iter()
                    .map(|ty| self.resolve(ty))
                    .collect::<Vec<_>>(),
                self.resolve(body_ty),
            ))
        })();
        self.env = saved_env;
        result
    }

    fn infer_expr(&mut self, expr: &Expr) -> Result<Type, TypeError> {
        match expr {
            Expr::Seq(items) => self.infer_pattern_items(items, "sequence items"),
            Expr::Stack(layers) => self.infer_homogeneous(layers, "`stack` layers"),
            Expr::Stream(items) => self.infer_homogeneous(items, "`stream` items"),
            Expr::Pipe { lhs, rhs } => {
                let lhs_ty = self.infer_expr(lhs)?;
                let rhs_ty = self.infer_expr(rhs)?;
                self.apply_argument(rhs_ty, lhs_ty)
            }
            Expr::Call { callee, args } => {
                let mut callee_ty = self.infer_expr(callee)?;
                for arg in args {
                    let arg_ty = self.infer_expr(arg)?;
                    callee_ty = self.apply_argument(callee_ty, arg_ty)?;
                }
                Ok(callee_ty)
            }
            Expr::At { start, pattern } => {
                let start_ty = self.infer_expr(start)?;
                self.unify(start_ty, Type::pattern(Type::Number))?;
                self.infer_expr(pattern)
            }
            Expr::Meter {
                beats,
                unit,
                pattern,
            } => {
                let beats_ty = self.infer_expr(beats)?;
                self.unify(beats_ty, Type::pattern(Type::Number))?;
                let unit_ty = self.infer_expr(unit)?;
                self.unify(unit_ty, Type::pattern(Type::Number))?;
                self.infer_expr(pattern)
            }
            Expr::Beat(value) => {
                let value_ty = self.infer_expr(value)?;
                self.unify(value_ty, Type::pattern(Type::Number))?;
                Ok(Type::pattern(Type::Number))
            }
            Expr::Section { pattern, cycles } => {
                let cycles_ty = self.infer_expr(cycles)?;
                self.unify(cycles_ty, Type::pattern(Type::Number))?;
                self.infer_expr(pattern)
            }
            Expr::SeqSections(items) => self.infer_homogeneous(items, "`seq_sections` items"),
            Expr::Group(items) => self.infer_pattern_items(items, "group items"),
            Expr::Ident(name) => self.infer_ident(name),
            Expr::Rest => Err(TypeError::new(
                "rest markers do not have a standalone type outside pattern sequences",
            )),
            Expr::Number(_) => Ok(Type::pattern(Type::Number)),
            Expr::String(_) => Ok(Type::String),
            Expr::Graph { .. } => Err(TypeError::new(
                "pedal graph bindings are parsed but not yet supported by type inference",
            )),
            Expr::Binary { .. } => Err(TypeError::new(
                "binary pedal expressions are parsed but not yet supported by type inference",
            )),
        }
    }

    fn infer_homogeneous(&mut self, items: &[Expr], context: &str) -> Result<Type, TypeError> {
        let Some((first, rest)) = items.split_first() else {
            return Err(TypeError::new(format!("{context} cannot be empty")));
        };

        let expected = self.infer_expr(first)?;
        for item in rest {
            let actual = self.infer_expr(item)?;
            self.unify(expected.clone(), actual.clone()).map_err(|_| {
                TypeError::new(format!(
                    "{context} must all have the same type; expected {}, found {}",
                    self.resolve(expected.clone()),
                    self.resolve(actual)
                ))
            })?;
        }

        Ok(self.resolve(expected))
    }

    fn infer_pattern_items(&mut self, items: &[Expr], context: &str) -> Result<Type, TypeError> {
        if items.is_empty() {
            return Err(TypeError::new(format!("{context} cannot be empty")));
        }

        let mut expected: Option<Type> = None;
        for item in items {
            if matches!(item, Expr::Rest) {
                continue;
            }

            let actual = self.infer_expr(item)?;
            if let Some(expected_ty) = expected.clone() {
                self.unify(expected_ty.clone(), actual.clone())
                    .map_err(|_| {
                        TypeError::new(format!(
                            "{context} must all have the same type; expected {}, found {}",
                            self.resolve(expected_ty),
                            self.resolve(actual)
                        ))
                    })?;
            } else {
                expected = Some(actual);
            }
        }

        if let Some(expected) = expected {
            Ok(self.resolve(expected))
        } else {
            Ok(Type::pattern(self.fresh_var_type()))
        }
    }

    fn infer_ident(&mut self, name: &str) -> Result<Type, TypeError> {
        if let Some(scheme) = self.env.get(name).cloned() {
            return Ok(self.instantiate(&scheme));
        }

        match parse_named_pitch_literal(name) {
            Ok(Some(_)) => Ok(Type::pattern(Type::Number)),
            Err(error) => Err(TypeError::new(error.to_string())),
            Ok(None) => Err(TypeError::new(format!("unresolved identifier `{name}`"))),
        }
    }

    fn apply_argument(&mut self, callee_ty: Type, arg_ty: Type) -> Result<Type, TypeError> {
        let param_ty = self.fresh_var_type();
        let ret_ty = self.fresh_var_type();
        self.unify(
            callee_ty,
            Type::function(vec![param_ty.clone()], ret_ty.clone()),
        )
        .map_err(|_| TypeError::new("attempted to call a non-function value"))?;
        self.unify(param_ty, arg_ty)?;
        Ok(self.resolve(ret_ty))
    }

    fn instantiate(&mut self, scheme: &TypeScheme) -> Type {
        let mut replacements = BTreeMap::new();
        for var in &scheme.vars {
            replacements.insert(*var, self.fresh_var_type());
        }
        substitute_scheme_vars(&scheme.ty, &replacements)
    }

    fn generalize(&self, ty: Type) -> TypeScheme {
        let ty = self.resolve(ty);
        let env_vars = self.free_vars_in_env();
        let vars = free_type_vars(&ty)
            .difference(&env_vars)
            .copied()
            .collect::<Vec<_>>();
        TypeScheme { vars, ty }
    }

    const fn fresh_var_type(&mut self) -> Type {
        let var = TypeVarId::new(self.next_var);
        self.next_var = self.next_var.saturating_add(1);
        Type::Var(var)
    }

    fn unify(&mut self, left: Type, right: Type) -> Result<(), TypeError> {
        let left = self.resolve(left);
        let right = self.resolve(right);

        match (left, right) {
            (Type::Var(left), Type::Var(right)) if left == right => Ok(()),
            (Type::Var(var), ty) | (ty, Type::Var(var)) => self.bind_var(var, ty),
            (Type::Pattern(left), Type::Pattern(right)) => self.unify(*left, *right),
            (Type::Function(left_args, left_ret), Type::Function(right_args, right_ret)) => {
                if left_args.len() != right_args.len() {
                    return Err(TypeError::new("function arity mismatch"));
                }
                for (left_arg, right_arg) in left_args.into_iter().zip(right_args) {
                    self.unify(left_arg, right_arg)?;
                }
                self.unify(*left_ret, *right_ret)
            }
            (Type::Sample, Type::Sample)
            | (Type::Note, Type::Note)
            | (Type::Number, Type::Number)
            | (Type::Duration, Type::Duration)
            | (Type::ArpDirection, Type::ArpDirection)
            | (Type::PitchClassSet, Type::PitchClassSet)
            | (Type::String, Type::String)
            | (Type::Unit, Type::Unit) => Ok(()),
            (left, right) => {
                if let Some((coerced_left, coerced_right)) = self.try_loose_coercion(&left, &right)
                {
                    return self.unify(coerced_left, coerced_right);
                }

                Err(TypeError::new(format!(
                    "type mismatch: expected {left}, found {right}"
                )))
            }
        }
    }

    fn bind_var(&mut self, var: TypeVarId, ty: Type) -> Result<(), TypeError> {
        if self.occurs(var, &ty) {
            return Err(TypeError::new(format!(
                "type variable t{} occurs within {}",
                var.0, ty
            )));
        }

        self.substitutions.insert(var, ty);
        Ok(())
    }

    fn occurs(&self, needle: TypeVarId, ty: &Type) -> bool {
        match self.resolve(ty.clone()) {
            Type::Var(var) => var == needle,
            Type::Pattern(inner) => self.occurs(needle, &inner),
            Type::Function(args, ret) => {
                args.iter().any(|arg| self.occurs(needle, arg)) || self.occurs(needle, &ret)
            }
            Type::Sample
            | Type::Note
            | Type::Number
            | Type::Duration
            | Type::ArpDirection
            | Type::PitchClassSet
            | Type::String
            | Type::Unit => false,
        }
    }

    fn resolve(&self, ty: Type) -> Type {
        match ty {
            Type::Var(var) => self
                .substitutions
                .get(&var)
                .cloned()
                .map_or(Type::Var(var), |bound| self.resolve(bound)),
            Type::Pattern(inner) => Type::pattern(self.resolve(*inner)),
            Type::Function(args, ret) => Type::function(
                args.into_iter().map(|arg| self.resolve(arg)).collect(),
                self.resolve(*ret),
            ),
            Type::Sample => Type::Sample,
            Type::Note => Type::Note,
            Type::Number => Type::Number,
            Type::Duration => Type::Duration,
            Type::ArpDirection => Type::ArpDirection,
            Type::PitchClassSet => Type::PitchClassSet,
            Type::String => Type::String,
            Type::Unit => Type::Unit,
        }
    }

    fn try_loose_coercion(&self, left: &Type, right: &Type) -> Option<(Type, Type)> {
        if self.mode != ReplMode::Loose {
            return None;
        }

        match (left, right) {
            (Type::Pattern(inner), other) if inner.as_ref() == other => {
                Some((left.clone(), left.clone()))
            }
            (other, Type::Pattern(inner)) if other == inner.as_ref() => {
                Some((right.clone(), right.clone()))
            }
            _ => None,
        }
    }

    fn free_vars_in_env(&self) -> BTreeSet<TypeVarId> {
        self.env
            .values()
            .flat_map(|scheme| {
                let mut vars = free_type_vars(&scheme.ty);
                for quantified in &scheme.vars {
                    vars.remove(quantified);
                }
                vars.into_iter()
            })
            .collect()
    }
}

fn substitute_scheme_vars(ty: &Type, replacements: &BTreeMap<TypeVarId, Type>) -> Type {
    match ty {
        Type::Pattern(inner) => Type::pattern(substitute_scheme_vars(inner, replacements)),
        Type::Function(args, ret) => Type::function(
            args.iter()
                .map(|arg| substitute_scheme_vars(arg, replacements))
                .collect(),
            substitute_scheme_vars(ret, replacements),
        ),
        Type::Var(var) => replacements.get(var).cloned().unwrap_or(Type::Var(*var)),
        Type::Sample => Type::Sample,
        Type::Note => Type::Note,
        Type::Number => Type::Number,
        Type::Duration => Type::Duration,
        Type::ArpDirection => Type::ArpDirection,
        Type::PitchClassSet => Type::PitchClassSet,
        Type::String => Type::String,
        Type::Unit => Type::Unit,
    }
}

fn free_type_vars(ty: &Type) -> BTreeSet<TypeVarId> {
    match ty {
        Type::Pattern(inner) => free_type_vars(inner),
        Type::Function(args, ret) => {
            let mut vars = BTreeSet::new();
            for arg in args {
                vars.extend(free_type_vars(arg));
            }
            vars.extend(free_type_vars(ret));
            vars
        }
        Type::Var(var) => BTreeSet::from([*var]),
        Type::Sample
        | Type::Note
        | Type::Number
        | Type::Duration
        | Type::ArpDirection
        | Type::PitchClassSet
        | Type::String
        | Type::Unit => BTreeSet::new(),
    }
}
