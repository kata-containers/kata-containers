use std::collections::{
    HashMap,
    BinaryHeap,
};
use std::collections::hash_map::Entry::{
    Occupied,
    Vacant,
};

use std::hash::Hash;

use scored::MinScored;
use super::visit::{
    Visitable,
    VisitMap,
    IntoEdges,
    EdgeRef,
};
use algo::Measure;

/// [Generic] Dijkstra's shortest path algorithm.
///
/// Compute the length of the shortest path from `start` to every reachable
/// node.
///
/// The graph should be `Visitable` and implement `IntoEdges`. The function
/// `edge_cost` should return the cost for a particular edge, which is used
/// to compute path costs. Edge costs must be non-negative.
///
/// If `goal` is not `None`, then the algorithm terminates once the `goal` node's
/// cost is calculated.
///
/// Returns a `HashMap` that maps `NodeId` to path cost.
pub fn dijkstra<G, F, K>(graph: G, start: G::NodeId, goal: Option<G::NodeId>,
                         mut edge_cost: F)
    -> HashMap<G::NodeId, K>
    where G: IntoEdges + Visitable,
          G::NodeId: Eq + Hash,
          F: FnMut(G::EdgeRef) -> K,
          K: Measure + Copy,
{
    let mut visited = graph.visit_map();
    let mut scores = HashMap::new();
    //let mut predecessor = HashMap::new();
    let mut visit_next = BinaryHeap::new();
    let zero_score = K::default();
    scores.insert(start, zero_score);
    visit_next.push(MinScored(zero_score, start));
    while let Some(MinScored(node_score, node)) = visit_next.pop() {
        if visited.is_visited(&node) {
            continue
        }
        if goal.as_ref() == Some(&node) {
            break
        }
        for edge in graph.edges(node) {
            let next = edge.target();
            if visited.is_visited(&next) {
                continue
            }
            let mut next_score = node_score + edge_cost(edge);
            match scores.entry(next) {
                Occupied(ent) => if next_score < *ent.get() {
                    *ent.into_mut() = next_score;
                    //predecessor.insert(next.clone(), node.clone());
                } else {
                    next_score = *ent.get();
                },
                Vacant(ent) => {
                    ent.insert(next_score);
                    //predecessor.insert(next.clone(), node.clone());
                }
            }
            visit_next.push(MinScored(next_score, next));
        }
        visited.visit(node);
    }
    scores
}
