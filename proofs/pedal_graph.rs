use vstd::prelude::*;

verus! {

// Source-order pedal graph model:
// - each binding occupies one source slot
// - `deps[i]` is the local binding referenced by slot `i`, or a negative value
//   when the edge is external rather than local
// - `feedback[i]` marks the one v1 exception where slot `i` may legally
//   reference itself through explicit `feedback(...)`

pub open spec fn is_local_reference(dep: int) -> bool {
    dep >= 0
}

pub open spec fn binding_dependency_is_legal(binding: int, dep: int, explicit_feedback: bool) -> bool {
    ||| !is_local_reference(dep)
    ||| (0 <= dep && dep < binding)
    ||| (explicit_feedback && dep == binding)
}

pub open spec fn feed_forward_binding_graph(deps: Seq<int>, feedback: Seq<bool>) -> bool {
    &&& deps.len() == feedback.len()
    &&& forall|i: int|
            #![auto]
            0 <= i < deps.len()
            ==> binding_dependency_is_legal(i, deps[i], feedback[i])
}

pub open spec fn canonical_source_order(deps: Seq<int>) -> Seq<int>
    recommends
        0 <= deps.len(),
{
    Seq::new(deps.len() as nat, |i: int| i)
}

pub open spec fn topological_evaluation_order(deps: Seq<int>, feedback: Seq<bool>, order: Seq<int>) -> bool {
    &&& order == canonical_source_order(deps)
    &&& forall|i: int|
            #![auto]
            0 <= i < deps.len()
            && is_local_reference(deps[i])
            && !(feedback[i] && deps[i] == i)
            ==> deps[i] < order[i]
}

pub open spec fn local_binding_is_defined_before_lowering(deps: Seq<int>, feedback: Seq<bool>) -> bool {
    forall|i: int|
        #![auto]
        0 <= i < deps.len()
        && is_local_reference(deps[i])
        && !(feedback[i] && deps[i] == i)
        ==> 0 <= deps[i] < i
}

pub open spec fn recursive_structure_enters_only_through_feedback(deps: Seq<int>, feedback: Seq<bool>) -> bool {
    forall|i: int|
        #![auto]
        0 <= i < deps.len()
        && is_local_reference(deps[i])
        && i <= deps[i]
        ==> feedback[i] && deps[i] == i
}

pub proof fn feed_forward_bindings_admit_a_topological_evaluation_order(deps: Seq<int>, feedback: Seq<bool>)
    requires
        feed_forward_binding_graph(deps, feedback),
    ensures
        topological_evaluation_order(deps, feedback, canonical_source_order(deps)),
{
}

pub proof fn every_referenced_local_binding_is_defined_before_lowering(deps: Seq<int>, feedback: Seq<bool>)
    requires
        feed_forward_binding_graph(deps, feedback),
    ensures
        local_binding_is_defined_before_lowering(deps, feedback),
{
}

pub proof fn recursive_structure_can_only_enter_through_explicit_feedback(deps: Seq<int>, feedback: Seq<bool>)
    requires
        feed_forward_binding_graph(deps, feedback),
    ensures
        recursive_structure_enters_only_through_feedback(deps, feedback),
{
}

pub proof fn canonical_order_uses_source_binding_indices(deps: Seq<int>)
    ensures
        canonical_source_order(deps).len() == deps.len(),
        forall|i: int| #![auto] 0 <= i < deps.len() ==> canonical_source_order(deps)[i] == i,
{
}

} // verus!

fn main() {}
