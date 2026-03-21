# Pitch Class Sets Design

**Date:** 2026-03-21

**Goal:** Replace stringly pitch collection lookup with first-class `pitch_class_set(...)` values so Orpheus users can define their own pitch vocabularies and use built-in collections through the same `degrees(...)` surface.

## Surface Syntax

Pitch collections become ordinary values in the top-level binding namespace:

```orpheus
hirajoshi = pitch_class_set(0 2 3 7 8)
iwato = pitch_class_set(0 1 5 6 10)

lead = degrees(hirajoshi, 0 1 2 4) |> transpose(60)
bass = degrees(aeolian, 0 2 4) |> transpose(36)
```

Canonical built-ins are no longer string-addressed. They become predefined bindings:

```orpheus
ionian
dorian
phrygian
mixolydian
aeolian
minor_pentatonic
```

So the new model is:

- `pitch_class_set(...)` constructs a first-class pitch collection value
- built-in collections are values of that same type
- `degrees(set, pattern)` consumes a collection value, not a string name

This removes the split-brain semantics where built-ins were magical strings while user-defined collections would have been ordinary bindings. The language becomes smaller and more coherent.

## Pitch Class Set Semantics

`pitch_class_set(...)` constructs a one-octave, root-relative pitch-class set.

Accepted inputs must be:

- whole-number semitone offsets
- strictly increasing
- rooted at `0`
- inside `[0, 11]`

Examples:

- valid: `pitch_class_set(0 2 3 7 8)`
- invalid: `pitch_class_set(2 3 7 8)` because it is not rooted at `0`
- invalid: `pitch_class_set(0 3 3 7)` because duplicates imply a non-set
- invalid: `pitch_class_set(0 7 3)` because ordering should be explicit, not silently normalized
- invalid: `pitch_class_set(0 2 12)` because octave leakage muddies degree semantics
- invalid: `pitch_class_set(0 2.5 7)` because fractional pitch classes are not in scope

The value represents an octave-local ordered set of semitone offsets, not a melodic pattern. That distinction matters: a degree pattern moves through time; a pitch-class set defines the index space used by `degrees(...)`.

## Runtime Model

This slice should introduce a first-class runtime value, something like:

- `Value::PitchClassSet(PitchClassSetValue)`

and retire the special `DegreeCollection` string lookup path.

The built-in canonical collections should be injected into the initial runtime environment through the existing builtin-value path, just like `bd`, `sn`, `fast`, and friends. That means `aeolian` becomes a normal identifier bound to a pitch-class-set value, not a string that `degrees(...)` interprets later.

`pitch_class_set(...)` should be a builtin constructor that:

1. accepts a numeric pattern describing one cycle of pitch classes,
2. materializes and validates its events as octave-local semitone offsets,
3. returns a `PitchClassSetValue`,
4. fails clearly if the input violates any of the set constraints.

`degrees(...)` should then accept only:

- a `PitchClassSetValue`
- a `Pattern<Number>` of integer degrees

and map the degrees using the same signed octave-carry law that already exists today. `transpose(...)` remains unchanged.

## Type System

This wants a real first-class type:

```text
PitchClassSet
```

The relevant builtin types become:

```text
pitch_class_set : Pattern<Number> -> PitchClassSet
degrees : PitchClassSet -> Pattern<Number> -> Pattern<Number>
transpose : Pattern<Number> -> Pattern<Number> -> Pattern<Number>
```

Canonical built-ins like `aeolian` and `dorian` should enter the type environment as monomorphic `PitchClassSet` values.

This is cleaner than disguising pitch-class sets as numeric patterns or strings. It lets inference reject nonsense like `degrees(bd, 0 1 2)` or `fast(2, aeolian)` with the right category boundary instead of evaluator sludge.

## Compatibility Cut

This slice should intentionally remove the old string path:

- supported: `degrees(aeolian, 0 2 4)`
- supported: `degrees(hirajoshi, 0 1 2 4)`
- removed: `degrees("aeolian", 0 2 4)`

Keeping both forms would preserve the impurity we are trying to remove. A short migration break now is better than carrying dual semantics around forever.

## Validation

Acceptance for v1:

- `pitch_class_set(0 2 3 7 8)` constructs a first-class value
- canonical built-ins exist as predefined pitch-class-set values
- `degrees(aeolian, 0 2 4)` and `degrees(hirajoshi, 0 1 2 4)` both work
- invalid pitch-class-set definitions fail clearly
- type inference understands `PitchClassSet`
- JSON export pins one melody built from a user-defined pitch-class set

## Proof Follow-Up

This design does not require a Verus proof in the same batch. The next obvious proof slice after implementation is the pitch-class-set validation law:

- rooted at `0`
- strictly increasing
- bounded within `[0, 11]`

But that should follow the runtime feature, not block it.
