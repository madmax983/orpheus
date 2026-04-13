use vstd::prelude::*;

verus! {

// Source-order pedal graph model:
// - each binding occupies one source slot
// - `graph[i]` lists every local binding index referenced by slot `i`, or a
//   negative value when the edge is external rather than local
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

pub open spec fn binding_dependencies_are_legal(
    binding: int,
    deps: Seq<int>,
    explicit_feedback: bool,
) -> bool {
    forall|dep_index: int|
        #![auto]
        0 <= dep_index < deps.len()
        ==> binding_dependency_is_legal(binding, deps[dep_index], explicit_feedback)
}

pub open spec fn feed_forward_binding_graph(graph: Seq<Seq<int>>, feedback: Seq<bool>) -> bool {
    &&& graph.len() == feedback.len()
    &&& forall|i: int|
            #![auto]
            0 <= i < graph.len()
            ==> binding_dependencies_are_legal(i, graph[i], feedback[i])
}

pub open spec fn canonical_source_order(graph: Seq<Seq<int>>) -> Seq<int>
    recommends
        0 <= graph.len(),
{
    Seq::new(graph.len() as nat, |i: int| i)
}

pub open spec fn topological_evaluation_order(
    graph: Seq<Seq<int>>,
    feedback: Seq<bool>,
    order: Seq<int>,
) -> bool {
    &&& order == canonical_source_order(graph)
    &&& forall|binding: int, dep_index: int|
            #![auto]
            0 <= binding < graph.len()
            && 0 <= dep_index < graph[binding].len()
            && is_local_reference(graph[binding][dep_index])
            && !(feedback[binding] && graph[binding][dep_index] == binding)
            ==> graph[binding][dep_index] < order[binding]
}

pub open spec fn local_binding_is_defined_before_lowering(
    graph: Seq<Seq<int>>,
    feedback: Seq<bool>,
) -> bool {
    forall|binding: int, dep_index: int|
        #![auto]
        0 <= binding < graph.len()
        && 0 <= dep_index < graph[binding].len()
        && is_local_reference(graph[binding][dep_index])
        && !(feedback[binding] && graph[binding][dep_index] == binding)
        ==> 0 <= graph[binding][dep_index] < binding
}

pub open spec fn recursive_structure_enters_only_through_feedback(
    graph: Seq<Seq<int>>,
    feedback: Seq<bool>,
) -> bool {
    forall|binding: int, dep_index: int|
        #![auto]
        0 <= binding < graph.len()
        && 0 <= dep_index < graph[binding].len()
        && is_local_reference(graph[binding][dep_index])
        && binding <= graph[binding][dep_index]
        ==> feedback[binding] && graph[binding][dep_index] == binding
}

pub proof fn feed_forward_bindings_admit_a_topological_evaluation_order(
    graph: Seq<Seq<int>>,
    feedback: Seq<bool>,
)
    requires
        feed_forward_binding_graph(graph, feedback),
    ensures
        topological_evaluation_order(graph, feedback, canonical_source_order(graph)),
{
}

pub proof fn every_referenced_local_binding_is_defined_before_lowering(
    graph: Seq<Seq<int>>,
    feedback: Seq<bool>,
)
    requires
        feed_forward_binding_graph(graph, feedback),
    ensures
        local_binding_is_defined_before_lowering(graph, feedback),
{
}

pub proof fn recursive_structure_can_only_enter_through_explicit_feedback(
    graph: Seq<Seq<int>>,
    feedback: Seq<bool>,
)
    requires
        feed_forward_binding_graph(graph, feedback),
    ensures
        recursive_structure_enters_only_through_feedback(graph, feedback),
{
}

pub proof fn canonical_order_uses_source_binding_indices(graph: Seq<Seq<int>>)
    ensures
        canonical_source_order(graph).len() == graph.len(),
        forall|i: int| #![auto] 0 <= i < graph.len() ==> canonical_source_order(graph)[i] == i,
{
}

} // verus!

fn main() {}
