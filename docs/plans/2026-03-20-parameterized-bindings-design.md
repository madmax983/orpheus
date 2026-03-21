# Parameterized Top-Level Bindings Design

**Date:** 2026-03-20  
**Status:** Validated  
**Scope:** `orpheus-lang`

## Goal

Add user-defined top-level parameterized bindings so musicians can build reusable
pattern vocabulary without expanding the builtin set for every new idiom.

This first slice adds:

- top-level parameterized bindings only
- curried calls with chained parenthesized application
- partial application for user-defined functions
- Hindley-Milner generalization for parameterized bindings

This slice explicitly does **not** add:

- anonymous lambdas
- local `let`
- closures over nested lexical scopes
- recursion or mutual recursion
- bare application syntax

## Chosen Surface Syntax

Parameterized bindings live only at the top level:

```orpheus
swing amt pat = pat |> shift(amt)
fill n pat = pat |> every(n, fast(2))
```

Calls are curried and always parenthesized:

```orpheus
groove = swing(0.125)(bd sn cp)
accent = every(4)(rev)(bd sn cp)
```

Pipe composition remains valid and useful:

```orpheus
groove = bd sn cp
  |> swing(0.125)
  |> fill(4)
```

Bare application such as `swing 0.125 (bd sn cp)` stays illegal. Orpheus
already uses juxtaposition for sequencing, and teaching the parser to guess
whether adjacent terms are notes or function arguments is a reliable way to
manufacture despair.

## Semantics

Top-level parameterized bindings desugar conceptually into nested unary
functions:

```orpheus
swing amt pat = pat |> shift(amt)
```

behaves like:

```text
swing = fn amt -> fn pat -> pat |> shift(amt)
```

The language does not need source-level `fn` syntax for this slice; the
desugaring is an implementation model.

Bindings remain source-ordered. A function body may reference:

- its own parameters
- earlier top-level bindings
- builtin values

A function body may not reference the binding currently being defined. This bans
recursion by construction and avoids fixpoint semantics in v1.

## Parser And AST Changes

`Stmt::Binding` should gain a `params: Vec<String>` field:

```text
Binding { name, params, expr }
```

Binding headers should accept:

```text
name = expr
name p1 p2 ... pn = expr
```

Expression parsing should support postfix call chaining on any callable
expression, not only bare identifiers. This allows:

- `fast(2)`
- `swing(0.125)(bd sn)`
- `(every(4))(rev)(bd sn)`

without changing sequence syntax.

The grammar should continue to reject bare application and keep juxtaposition
reserved for pattern sequencing.

## Runtime Model

`Value::Function(BuiltinFn)` is too narrow once users can define functions. The
runtime should introduce a general callable representation:

- builtin callable with stored bound arguments
- user callable with remaining parameters, body AST, and captured environment

User callables only need top-level capture semantics in this slice. When a
parameterized binding is defined, it captures the bindings visible at that
definition site. Application binds one argument at a time:

- if arguments are still missing, return another callable
- once all parameters are bound, evaluate the stored body in the extended
  environment

This mirrors builtin partial application so user functions and builtins obey the
same mental model.

## Type System Changes

The current inferencer already applies arguments one at a time, which fits
curried calls well. The missing piece is generalization.

For parameterized bindings:

- infer the binding body using parameter names mapped to fresh type variables
- resolve the resulting function type
- generalize unconstrained type variables into a `TypeScheme`
- insert the generalized scheme into the type environment

At use sites, user-defined functions should be instantiated the same way builtin
schemes are instantiated. This enables reusable polymorphic transforms instead
of locking every user function to its first call site.

Example target shape:

```orpheus
swing amt pat = pat |> shift(amt)
```

should infer roughly:

```text
Pattern<Number> -> Pattern<t> -> Pattern<t>
```

subject to whatever constraints the body introduces.

## Error Handling And Diagnostics

V1 should produce direct errors for the following cases:

- self-reference inside a binding body
- unresolved names in strict mode
- attempts to call non-function values
- too many arguments applied to a fully saturated callable
- invalid parameter names or duplicate parameters in a binding header

Diagnostics do not need to be fancy yet; they do need to be specific.

## Tests And Rollout

Deliver this as a narrow vertical slice:

1. Add parser tests for parameterized headers and chained calls.
2. Add type inference tests for generalized curried bindings.
3. Add runtime tests for partial application, full application, and pipe usage.
4. Add regression tests proving sequencing syntax still parses as sequencing.
5. Add error tests for recursion rejection and duplicate parameters.

Acceptance criteria:

- `foo x y = ...` parses successfully
- `foo(1)(2)` parses as nested calls
- user-defined functions support partial application
- user-defined functions compose with pipes
- parameterized bindings are generalized, not monomorphic
- self-recursion is rejected clearly
- existing sequence grammar is unchanged

## Recommended Implementation Strategy

Use an explicit user-function runtime representation plus generalized type
schemes.

Do not fake this with parser-only desugaring into builtin-shaped placeholders.
That shortcut buys a fast patch and a slower future. Once lambdas arrive, the
hack will start shedding teeth.
