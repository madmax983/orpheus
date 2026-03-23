use vstd::prelude::*;

verus! {

// Phase-1 routing model:
// - tracks are stable sources
// - buses are named shared destinations
// - master is the final sink
// - bus-to-bus and bus-to-track routing are forbidden
// - a complete route is one whose final node has no legal outgoing edge

pub open spec fn is_track_node(node: int) -> bool {
    node == 0
}

pub open spec fn is_bus_node(node: int) -> bool {
    node == 1
}

pub open spec fn is_master_node(node: int) -> bool {
    node == 2
}

pub open spec fn phase1_node_is_valid(node: int) -> bool {
    is_track_node(node) || is_bus_node(node) || is_master_node(node)
}

pub open spec fn phase1_edge_allowed(from: int, to: int) -> bool {
    (is_track_node(from) && (is_bus_node(to) || is_master_node(to)))
        || (is_bus_node(from) && is_master_node(to))
}

pub open spec fn phase1_route_hop_allowed(route: Seq<int>, index: int) -> bool
    recommends
        0 <= index && index + 1 < route.len(),
{
    phase1_edge_allowed(route[index], route[index + 1])
}

pub open spec fn phase1_route_nodes_are_valid(route: Seq<int>) -> bool {
    route.len() > 0
        && forall|i: int| 0 <= i && i < route.len() ==> #[trigger] phase1_node_is_valid(route[i])
}

pub open spec fn phase1_route_edges_are_valid(route: Seq<int>) -> bool {
    forall|i: int| 0 <= i && i + 1 < route.len() ==> #[trigger] phase1_route_hop_allowed(route, i)
}

pub open spec fn phase1_route_shape(route: Seq<int>) -> bool {
    phase1_route_nodes_are_valid(route) && phase1_route_edges_are_valid(route)
}

pub open spec fn node_has_phase1_successor(node: int) -> bool {
    exists|to: int| #[trigger] phase1_node_is_valid(to) && #[trigger] phase1_edge_allowed(node, to)
}

pub open spec fn phase1_complete_route_from_track(route: Seq<int>) -> bool {
    route.len() > 1
        && phase1_route_shape(route)
        && is_track_node(route[0])
        && !node_has_phase1_successor(route[route.len() - 1])
}

pub open spec fn route_terminates_at_master(route: Seq<int>) -> bool {
    route.len() > 0 && is_master_node(route[route.len() - 1])
}

pub open spec fn route_step_is_not_bus_to_bus(route: Seq<int>, index: int) -> bool
    recommends
        0 <= index && index + 1 < route.len(),
{
    !(is_bus_node(route[index]) && is_bus_node(route[index + 1]))
}

pub open spec fn route_has_no_bus_to_bus_hops(route: Seq<int>) -> bool {
    forall|i: int| 0 <= i && i + 1 < route.len() ==> #[trigger] route_step_is_not_bus_to_bus(route, i)
}

pub proof fn phase1_track_nodes_have_successors()
    ensures
        node_has_phase1_successor(0),
{
    assert(phase1_node_is_valid(1));
    assert(phase1_edge_allowed(0, 1));
}

pub proof fn phase1_bus_nodes_have_successors()
    ensures
        node_has_phase1_successor(1),
{
    assert(phase1_node_is_valid(2));
    assert(phase1_edge_allowed(1, 2));
}

pub proof fn phase1_master_has_no_successors()
    ensures
        !node_has_phase1_successor(2),
{
    assert forall|to: int| #![trigger phase1_edge_allowed(2, to)] phase1_node_is_valid(to) implies !phase1_edge_allowed(2, to) by {
    };
}

pub proof fn phase1_route_shape_allows_hop(route: Seq<int>, index: int)
    requires
        phase1_route_edges_are_valid(route),
        0 <= index && index + 1 < route.len(),
    ensures
        phase1_edge_allowed(route[index], route[index + 1]),
{
    assert(phase1_route_hop_allowed(route, index));
    assert(phase1_edge_allowed(route[index], route[index + 1]));
}

pub proof fn phase1_complete_routes_end_at_master(route: Seq<int>)
    requires
        phase1_complete_route_from_track(route),
    ensures
        route_terminates_at_master(route),
{
    let last = route[route.len() - 1];
    assert(phase1_route_nodes_are_valid(route));
    assert(phase1_node_is_valid(last));

    if is_track_node(last) {
        phase1_track_nodes_have_successors();
        assert(node_has_phase1_successor(last));
        assert(false);
    } else if is_bus_node(last) {
        phase1_bus_nodes_have_successors();
        assert(node_has_phase1_successor(last));
        assert(false);
    } else {
        assert(is_master_node(last));
    }
}

pub proof fn phase1_routes_have_no_bus_to_bus_hops(route: Seq<int>)
    requires
        phase1_route_edges_are_valid(route),
    ensures
        route_has_no_bus_to_bus_hops(route),
{
    assert forall|i: int| 0 <= i && i + 1 < route.len() implies route_step_is_not_bus_to_bus(route, i)
    by {
        if is_bus_node(route[i]) && is_bus_node(route[i + 1]) {
            phase1_route_shape_allows_hop(route, i);
            assert(false);
        }
    };
}

pub proof fn phase1_bus_nodes_do_not_feed_tracks(from: int, to: int)
    requires
        is_bus_node(from),
        is_track_node(to),
    ensures
        !phase1_edge_allowed(from, to),
{
    assert(!phase1_edge_allowed(from, to));
}

} // verus!

fn main() {}
