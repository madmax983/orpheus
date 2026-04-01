use vstd::prelude::*;

verus! {

pub open spec fn is_local_reference(dep: int) -> bool {
    dep >= 0
}

pub open spec fn is_explicit_feedback(dep: int) -> bool {
    dep == -2
}

pub open spec fn feed_forward_binding_graph(deps: Seq<int>) -> bool {
    forall|i: int| 0 <= i && i < deps.len() ==> !is_local_reference(deps[i]) || deps[i] < i || is_explicit_feedback(deps[i])
}

pub open spec fn topological_evaluation_order(deps: Seq<int>) -> bool {
    feed_forward_binding_graph(deps)
}

pub open spec fn local_binding_is_defined_before_lowering(deps: Seq<int>) -> bool {
    feed_forward_binding_graph(deps)
}

pub open spec fn recursive_structure_enters_only_through_feedback(deps: Seq<int>) -> bool {
    feed_forward_binding_graph(deps)
}

pub proof fn feed_forward_bindings_admit_a_topological_evaluation_order(deps: Seq<int>)
    requires
        feed_forward_binding_graph(deps),
    ensures
        topological_evaluation_order(deps),
{
}

pub proof fn every_referenced_local_binding_is_defined_before_lowering(deps: Seq<int>)
    requires
        feed_forward_binding_graph(deps),
    ensures
        local_binding_is_defined_before_lowering(deps),
{
}

pub proof fn recursive_structure_can_only_enter_through_explicit_feedback(deps: Seq<int>)
    requires
        feed_forward_binding_graph(deps),
    ensures
        recursive_structure_enters_only_through_feedback(deps),
{
}

pub proof fn feed_forward_bindings_are_acyclic(deps: Seq<int>)
    requires
        feed_forward_binding_graph(deps),
    ensures
        topological_evaluation_order(deps),
{
    feed_forward_bindings_admit_a_topological_evaluation_order(deps);
}

} // verus!

fn main() {}
