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
use crate::ast::{BinaryOp, Expr, Module, Stmt, binding_expr_self_references};
use crate::builtins::markov_state_count;
use crate::diagnostics::{ParseError, TypeError};
use crate::parser::parse_module;
use crate::pitch::parse_named_pitch_literal;
use crate::types::env::{
    TypeEnv, TypeScheme, choose_scheme, euclid_full_scheme, euclid_scheme, markov_scheme,
    pattern_concat_scheme, wchoose_scheme, wrandcat_scheme,
};
use crate::types::{Type, TypeVarId, TypedModule};

/// Infers the types of top-level bindings in an Orpheus module.
///
///
/// # Examples
///
/// ```
/// use orpheus_lang::{infer_module, ReplMode};
///
/// let typed = infer_module("x = bd sn", ReplMode::Loose).unwrap();
/// assert!(typed.contains_key("x"));
/// ```
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
///
/// # Examples
///
/// ```
/// use std::collections::BTreeMap;
/// use orpheus_lang::{eval_into_bindings, ReplMode};
///
/// let mut env = BTreeMap::new();
/// eval_into_bindings("x = bd sn", ReplMode::Loose, &mut env).unwrap();
/// assert!(env.contains_key("x"));
/// ```
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

        let mut old_bindings = Vec::with_capacity(params.len());
        let mut param_types = Vec::with_capacity(params.len());
        for param in params {
            let ty = self.fresh_var_type();
            let old_binding = self.env
                .insert(param.clone(), TypeScheme::monomorphic(ty.clone()));
            old_bindings.push((param.clone(), old_binding));
            param_types.push(ty);
        }

        let body_ty = self.infer_expr(expr);

        // Restore the old bindings instead of cloning the entire environment
        for (param, old_binding) in old_bindings.into_iter().rev() {
            if let Some(old) = old_binding {
                self.env.insert(param, old);
            } else {
                self.env.remove(&param);
            }
        }

        let body_ty = body_ty?;
        Ok(Type::curried(
            param_types
                .into_iter()
                .map(|ty| self.resolve(ty))
                .collect::<Vec<_>>(),
            self.resolve(body_ty),
        ))
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
                if let Some(ty) = self.infer_variadic_call(callee, args)? {
                    return Ok(ty);
                }
                let mut callee_ty = self.infer_expr(callee)?;
                if matches!(self.resolve(callee_ty.clone()), Type::Pattern(_))
                    && (2..=3).contains(&args.len())
                {
                    // Inline euclid sugar `bd(3, 8[, rot])`: "calling" a
                    // pattern euclidean-gates it, so the call keeps the
                    // pattern's own type and takes numeric arguments.
                    for arg in args {
                        let actual = self.infer_expr(arg)?;
                        self.unify(actual.clone(), Type::pattern(Type::Number))
                            .map_err(|_| {
                                TypeError::new(format!(
                                    "inline euclid arguments must all be numbers; found {}",
                                    self.resolve(actual)
                                ))
                            })?;
                    }
                    return Ok(self.resolve(callee_ty));
                }
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
            Expr::Alternation(items) => self.infer_pattern_items(items, "alternation items"),
            Expr::Modified { inner, .. } => {
                self.infer_pattern_items(std::slice::from_ref(inner), "step operator operand")
            }
            Expr::Polymeter { groups, .. } => self.infer_polymeter_groups(groups),
            Expr::Ident(name) => self.infer_ident(name),
            Expr::Rest => Err(TypeError::new(
                "rest markers do not have a standalone type outside pattern sequences",
            )),
            Expr::Number(_) => Ok(Type::pattern(Type::Number)),
            Expr::String(_) => Ok(Type::String),
            Expr::Graph { .. } => Ok(Type::Pedal),
            Expr::Voice { .. } => Ok(Type::Voice),
            Expr::Binary { lhs, op, rhs } => self.infer_binary_expr(lhs, *op, rhs),
        }
    }

    fn infer_binary_expr(
        &mut self,
        lhs: &Expr,
        op: BinaryOp,
        rhs: &Expr,
    ) -> Result<Type, TypeError> {
        match op {
            BinaryOp::Add | BinaryOp::Mul => {
                self.require_numeric_binary_operand(lhs)?;
                self.require_numeric_binary_operand(rhs)?;
                Ok(Type::pattern(Type::Number))
            }
            BinaryOp::Assign => {
                if !matches!(lhs, Expr::Ident(_)) {
                    return Err(TypeError::new(
                        "named arguments require an identifier on the left-hand side",
                    ));
                }
                self.infer_expr(rhs)
            }
        }
    }

    fn require_numeric_binary_operand(&mut self, expr: &Expr) -> Result<(), TypeError> {
        let ty = self.infer_expr(expr)?;
        let resolved = self.resolve(ty.clone());

        match resolved {
            Type::Pattern(inner) if inner.as_ref() == &Type::Number => Ok(()),
            Type::Var(_) => {
                self.unify(ty, Type::pattern(Type::Number))?;
                Ok(())
            }
            other => Err(TypeError::new(format!(
                "type mismatch: expected Number, found {other}"
            ))),
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

    /// Types a `{a b, c d e}` polymeter: every subsequence is a pattern item
    /// list and all subsequences must agree on one element type.
    fn infer_polymeter_groups(&mut self, groups: &[Vec<Expr>]) -> Result<Type, TypeError> {
        let mut expected: Option<Type> = None;
        for group in groups {
            let actual = self.infer_pattern_items(group, "polymeter items")?;
            if let Some(expected_ty) = expected.clone() {
                self.unify(expected_ty.clone(), actual.clone()).map_err(|_| {
                    TypeError::new(format!(
                        "polymeter subsequences must all have the same type; expected {}, found {}",
                        self.resolve(expected_ty),
                        self.resolve(actual)
                    ))
                })?;
            } else {
                expected = Some(actual);
            }
        }

        expected.map_or_else(
            || Err(TypeError::new("polymeter cannot be empty")),
            |expected| Ok(self.resolve(expected)),
        )
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
            // A voice binding used inside a pattern is a sample-like token:
            // the engine resolves it through the graph voice bank.
            let actual = if matches!(self.resolve(actual.clone()), Type::Voice) {
                Type::pattern(Type::Sample)
            } else {
                actual
            };
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

    /// Types variadic builtin calls (`cat`/`slowcat`/`randcat`, `choose`,
    /// `wchoose`, `wrandcat`) with more arguments than their base environment
    /// scheme covers.
    ///
    /// The environment schemes only cover the curried base-arity forms;
    /// larger argument lists are checked here. Calls where the builtin has
    /// been shadowed by a user binding fall through to the generic
    /// application path.
    fn infer_variadic_call(
        &mut self,
        callee: &Expr,
        args: &[Expr],
    ) -> Result<Option<Type>, TypeError> {
        let Expr::Ident(name) = callee else {
            return Ok(None);
        };
        match name.as_str() {
            "cat" | "slowcat" | "randcat" | "pchoose" if args.len() > 2 => {
                if self.env.get(name) != Some(&pattern_concat_scheme(TypeVarId::new(0))) {
                    return Ok(None);
                }
                let context = format!("`{name}` patterns");
                let ty = self.infer_homogeneous(args, &context)?;
                let element = self.fresh_var_type();
                self.unify(ty.clone(), Type::pattern(element))?;
                Ok(Some(self.resolve(ty)))
            }
            "choose" if args.len() > 2 => {
                if self.env.get(name) != Some(&choose_scheme()) {
                    return Ok(None);
                }
                self.infer_number_pattern_arguments(args, "`choose` values")
            }
            "wchoose" if args.len() > 4 => {
                if self.env.get(name) != Some(&wchoose_scheme()) {
                    return Ok(None);
                }
                self.infer_number_pattern_arguments(args, "`wchoose` value/weight pairs")
            }
            "wrandcat" | "wpchoose" if args.len() > 4 => {
                if self.env.get(name) != Some(&wrandcat_scheme(TypeVarId::new(0))) {
                    return Ok(None);
                }
                self.infer_wrandcat_pairs(name, args)
            }
            "markov" if args.len() > 6 => {
                if self.env.get(name) != Some(&markov_scheme(TypeVarId::new(0))) {
                    return Ok(None);
                }
                self.infer_markov_blocks(args)
            }
            "euclid" | "euclid_inv" if args.len() > 2 => {
                if self.env.get(name) != Some(&euclid_scheme()) {
                    return Ok(None);
                }
                if args.len() > 3 {
                    return Err(TypeError::new(format!(
                        "`{name}` accepts at most 3 arguments (pulses, steps, rotation)"
                    )));
                }
                let context = format!("`{name}` arguments");
                self.infer_number_pattern_arguments(args, &context)
            }
            "euclid_full" if args.len() > 4 => {
                if self.env.get(name) != Some(&euclid_full_scheme(TypeVarId::new(0))) {
                    return Ok(None);
                }
                if args.len() > 5 {
                    return Err(TypeError::new(
                        "`euclid_full` accepts at most 5 arguments \
                         (pulses, steps, rotation, hits, rests)",
                    ));
                }
                self.infer_euclid_full_with_rotation(args)
            }
            _ => Ok(None),
        }
    }

    /// Types the five-argument `euclid_full(pulses, steps, rotation, hits,
    /// rests)` form: the first three arguments must be number patterns and
    /// the two trailing patterns must agree on one pattern type, which the
    /// call returns.
    fn infer_euclid_full_with_rotation(
        &mut self,
        args: &[Expr],
    ) -> Result<Option<Type>, TypeError> {
        for arg in &args[..3] {
            let actual = self.infer_expr(arg)?;
            self.unify(actual.clone(), Type::pattern(Type::Number))
                .map_err(|_| {
                    TypeError::new(format!(
                        "`euclid_full` pulses, steps, and rotation must be numbers; found {}",
                        self.resolve(actual)
                    ))
                })?;
        }

        let ty = self.infer_homogeneous(&args[3..], "`euclid_full` hit and rest patterns")?;
        let element = self.fresh_var_type();
        self.unify(ty.clone(), Type::pattern(element))?;
        Ok(Some(self.resolve(ty)))
    }

    /// Types variadic `wrandcat`/`wpchoose` `(p1, w1, p2, w2, ...)` calls:
    /// the even-indexed pattern arguments must agree on one pattern type,
    /// the odd-indexed weights must be numbers, and the call returns the
    /// shared pattern type.
    fn infer_wrandcat_pairs(
        &mut self,
        name: &str,
        args: &[Expr],
    ) -> Result<Option<Type>, TypeError> {
        if !args.len().is_multiple_of(2) {
            return Err(TypeError::new(format!(
                "`{name}` requires interleaved pattern/weight pairs",
            )));
        }

        let mut pattern_ty: Option<Type> = None;
        for (index, arg) in args.iter().enumerate() {
            let actual = self.infer_expr(arg)?;
            if !index.is_multiple_of(2) {
                self.unify(actual.clone(), Type::pattern(Type::Number))
                    .map_err(|_| {
                        TypeError::new(format!(
                            "`{name}` weights must be numbers; found {}",
                            self.resolve(actual)
                        ))
                    })?;
            } else if let Some(expected) = pattern_ty.clone() {
                self.unify(expected.clone(), actual.clone()).map_err(|_| {
                    TypeError::new(format!(
                        "`{name}` patterns must all have the same type; expected {}, found {}",
                        self.resolve(expected),
                        self.resolve(actual)
                    ))
                })?;
            } else {
                pattern_ty = Some(actual);
            }
        }

        let ty = pattern_ty.ok_or_else(|| {
            TypeError::new(format!(
                "`{name}` requires at least one pattern/weight pair"
            ))
        })?;
        let element = self.fresh_var_type();
        self.unify(ty.clone(), Type::pattern(element))?;
        Ok(Some(self.resolve(ty)))
    }

    /// Types variadic `markov(s0, w0_0, ..., w0_{k-1}, s1, ...)` calls: the
    /// argument count must be `k * (k + 1)` for `k >= 2` states, the leading
    /// pattern of every state block must agree on one pattern type, and each
    /// block's `k` trailing transition weights must be numbers.
    fn infer_markov_blocks(&mut self, args: &[Expr]) -> Result<Option<Type>, TypeError> {
        let Some(state_count) = markov_state_count(args.len()) else {
            return Err(TypeError::new(
                "`markov` requires each state pattern followed by its transition weights \
                 (k * (k + 1) arguments for k >= 2 states)",
            ));
        };

        let mut pattern_ty: Option<Type> = None;
        for (index, arg) in args.iter().enumerate() {
            let actual = self.infer_expr(arg)?;
            if index.is_multiple_of(state_count + 1) {
                if let Some(expected) = pattern_ty.clone() {
                    self.unify(expected.clone(), actual.clone()).map_err(|_| {
                        TypeError::new(format!(
                            "`markov` state patterns must all have the same type; \
                             expected {}, found {}",
                            self.resolve(expected),
                            self.resolve(actual)
                        ))
                    })?;
                } else {
                    pattern_ty = Some(actual);
                }
            } else {
                self.unify(actual.clone(), Type::pattern(Type::Number))
                    .map_err(|_| {
                        TypeError::new(format!(
                            "`markov` transition weights must be numbers; found {}",
                            self.resolve(actual)
                        ))
                    })?;
            }
        }

        let ty =
            pattern_ty.ok_or_else(|| TypeError::new("`markov` requires at least two states"))?;
        let element = self.fresh_var_type();
        self.unify(ty.clone(), Type::pattern(element))?;
        Ok(Some(self.resolve(ty)))
    }

    /// Checks every argument against `Pattern<Number>` and returns
    /// `Pattern<Number>` as the call's type.
    fn infer_number_pattern_arguments(
        &mut self,
        args: &[Expr],
        context: &str,
    ) -> Result<Option<Type>, TypeError> {
        for arg in args {
            let actual = self.infer_expr(arg)?;
            self.unify(actual.clone(), Type::pattern(Type::Number))
                .map_err(|_| {
                    TypeError::new(format!(
                        "{context} must all be numbers; found {}",
                        self.resolve(actual)
                    ))
                })?;
        }
        Ok(Some(Type::pattern(Type::Number)))
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
        let arg_ty = self.coerce_nullary_function_argument(&param_ty, arg_ty);
        self.unify(param_ty, arg_ty)?;
        Ok(self.resolve(ret_ty))
    }

    /// Coerces a zero-arity function argument (e.g. bare `rand`, typed
    /// `() -> Pattern<Number>`) to its result type when the parameter does
    /// not itself expect a function, so `rand |> segment(8)` and
    /// `segment(4, rand)` type-check without writing `rand()`. Mirrors the
    /// evaluator, which invokes saturated zero-arity builtins in pattern
    /// position.
    fn coerce_nullary_function_argument(&self, param_ty: &Type, arg_ty: Type) -> Type {
        match self.resolve(param_ty.clone()) {
            Type::Function(..) | Type::Var(_) => arg_ty,
            _ => match self.resolve(arg_ty.clone()) {
                Type::Function(args, ret) if args.is_empty() => *ret,
                _ => arg_ty,
            },
        }
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
            | (Type::Pedal, Type::Pedal)
            | (Type::Voice, Type::Voice)
            | (Type::Plugin, Type::Plugin)
            | (Type::Note, Type::Note)
            | (Type::Number, Type::Number)
            | (Type::Duration, Type::Duration)
            | (Type::ArpDirection, Type::ArpDirection)
            | (Type::PitchClassSet, Type::PitchClassSet)
            | (Type::Tuning, Type::Tuning)
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
            | Type::Pedal
            | Type::Voice
            | Type::Plugin
            | Type::Note
            | Type::Number
            | Type::Duration
            | Type::ArpDirection
            | Type::PitchClassSet
            | Type::Tuning
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
            Type::Pedal => Type::Pedal,
            Type::Voice => Type::Voice,
            Type::Plugin => Type::Plugin,
            Type::Note => Type::Note,
            Type::Number => Type::Number,
            Type::Duration => Type::Duration,
            Type::ArpDirection => Type::ArpDirection,
            Type::PitchClassSet => Type::PitchClassSet,
            Type::Tuning => Type::Tuning,
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
        Type::Pedal => Type::Pedal,
        Type::Voice => Type::Voice,
        Type::Plugin => Type::Plugin,
        Type::Note => Type::Note,
        Type::Number => Type::Number,
        Type::Duration => Type::Duration,
        Type::ArpDirection => Type::ArpDirection,
        Type::PitchClassSet => Type::PitchClassSet,
        Type::Tuning => Type::Tuning,
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
        | Type::Pedal
        | Type::Voice
        | Type::Plugin
        | Type::Note
        | Type::Number
        | Type::Duration
        | Type::ArpDirection
        | Type::PitchClassSet
        | Type::Tuning
        | Type::String
        | Type::Unit => BTreeSet::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::ParseError;
    #[test]
    fn test_type_error_from_parse_error() {
        let parse_err = ParseError::new("mock parse error");
        let type_err: TypeError = parse_err.into();
        assert!(type_err.to_string().contains("mock parse error"));
    }
}
