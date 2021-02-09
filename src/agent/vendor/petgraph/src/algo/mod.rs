//! Graph algorithms.
//!
//! It is a goal to gradually migrate the algorithms to be based on graph traits
//! so that they are generally applicable. For now, some of these still require
//! the `Graph` type.

pub mod dominators;

use std::collections::BinaryHeap;
use std::cmp::min;

use prelude::*;

use super::{
    EdgeType,
};
use scored::MinScored;
use super::visit::{
    GraphRef,
    GraphBase,
    Visitable,
    VisitMap,
    IntoNeighbors,
    IntoNeighborsDirected,
    IntoNodeIdentifiers,
    NodeCount,
    NodeIndexable,
    NodeCompactIndexable,
    IntoEdgeReferences,
    IntoEdges,
    Reversed,
};
use super::unionfind::UnionFind;
use super::graph::{
    IndexType,
};
use visit::{Data, NodeRef, IntoNodeReferences};
use data::{
    Element,
};

pub use super::isomorphism::{
    is_isomorphic,
    is_isomorphic_matching,
};
pub use super::dijkstra::dijkstra;
pub use super::astar::astar;

/// [Generic] Return the number of connected components of the graph.
///
/// For a directed graph, this is the *weakly* connected components.
pub fn connected_components<G>(g: G) -> usize
    where G: NodeCompactIndexable + IntoEdgeReferences,
{
    let mut vertex_sets = UnionFind::new(g.node_bound());
    for edge in g.edge_references() {
        let (a, b) = (edge.source(), edge.target());

        // union the two vertices of the edge
        vertex_sets.union(g.to_index(a), g.to_index(b));
    }
    let mut labels = vertex_sets.into_labeling();
    labels.sort();
    labels.dedup();
    labels.len()
}


/// [Generic] Return `true` if the input graph contains a cycle.
///
/// Always treats the input graph as if undirected.
pub fn is_cyclic_undirected<G>(g: G) -> bool
    where G: NodeIndexable + IntoEdgeReferences
{
    let mut edge_sets = UnionFind::new(g.node_bound());
    for edge in g.edge_references() {
        let (a, b) = (edge.source(), edge.target());

        // union the two vertices of the edge
        //  -- if they were already the same, then we have a cycle
        if !edge_sets.union(g.to_index(a), g.to_index(b)) {
            return true
        }
    }
    false
}


/// [Generic] Perform a topological sort of a directed graph.
///
/// If the graph was acyclic, return a vector of nodes in topological order:
/// each node is ordered before its successors.
/// Otherwise, it will return a `Cycle` error. Self loops are also cycles.
///
/// To handle graphs with cycles, use the scc algorithms or `DfsPostOrder`
/// instead of this function.
///
/// If `space` is not `None`, it is used instead of creating a new workspace for
/// graph traversal. The implementation is iterative.
pub fn toposort<G>(g: G, space: Option<&mut DfsSpace<G::NodeId, G::Map>>)
    -> Result<Vec<G::NodeId>, Cycle<G::NodeId>>
    where G: IntoNeighborsDirected + IntoNodeIdentifiers + Visitable,
{
    // based on kosaraju scc
    with_dfs(g, space, |dfs| {
        dfs.reset(g);
        let mut finished = g.visit_map();

        let mut finish_stack = Vec::new();
        for i in g.node_identifiers() {
            if dfs.discovered.is_visited(&i) {
                continue;
            }
            dfs.stack.push(i);
            while let Some(&nx) = dfs.stack.last() {
                if dfs.discovered.visit(nx) {
                    // First time visiting `nx`: Push neighbors, don't pop `nx`
                    for succ in g.neighbors(nx) {
                        if succ == nx {
                            // self cycle
                            return Err(Cycle(nx));
                        }
                        if !dfs.discovered.is_visited(&succ) {
                            dfs.stack.push(succ);
                        } 
                    }
                } else {
                    dfs.stack.pop();
                    if finished.visit(nx) {
                        // Second time: All reachable nodes must have been finished
                        finish_stack.push(nx);
                    }
                }
            }
        }
        finish_stack.reverse();

        dfs.reset(g);
        for &i in &finish_stack {
            dfs.move_to(i);
            let mut cycle = false;
            while let Some(j) = dfs.next(Reversed(g)) {
                if cycle {
                    return Err(Cycle(j));
                }
                cycle = true;
            }
        }

        Ok(finish_stack)
    })
}

/// [Generic] Return `true` if the input directed graph contains a cycle.
///
/// This implementation is recursive; use `toposort` if an alternative is
/// needed.
pub fn is_cyclic_directed<G>(g: G) -> bool
    where G: IntoNodeIdentifiers + IntoNeighbors + Visitable,
{
    use visit::{depth_first_search, DfsEvent};

    depth_first_search(g, g.node_identifiers(), |event| {
        match event {
            DfsEvent::BackEdge(_, _) => Err(()),
            _ => Ok(()),
        }
    }).is_err()
}

type DfsSpaceType<G> = DfsSpace<<G as GraphBase>::NodeId, <G as Visitable>::Map>;

/// Workspace for a graph traversal.
#[derive(Clone, Debug)]
pub struct DfsSpace<N, VM> {
    dfs: Dfs<N, VM>,
}

impl<N, VM> DfsSpace<N, VM>
    where N: Copy + PartialEq,
          VM: VisitMap<N>,
{
    pub fn new<G>(g: G) -> Self
        where G: GraphRef + Visitable<NodeId=N, Map=VM>,
    {
        DfsSpace {
            dfs: Dfs::empty(g)
        }
    }
}

impl<N, VM> Default for DfsSpace<N, VM>
    where VM: VisitMap<N> + Default,
{
    fn default() -> Self {
        DfsSpace {
            dfs: Dfs {
                stack: <_>::default(),
                discovered: <_>::default(),
            }
        }
    }
}

/// Create a Dfs if it's needed
fn with_dfs<G, F, R>(g: G, space: Option<&mut DfsSpaceType<G>>, f: F) -> R
    where G: GraphRef + Visitable,
          F: FnOnce(&mut Dfs<G::NodeId, G::Map>) -> R
{
    let mut local_visitor;
    let dfs = if let Some(v) = space { &mut v.dfs } else {
        local_visitor = Dfs::empty(g);
        &mut local_visitor
    };
    f(dfs)
}

/// [Generic] Check if there exists a path starting at `from` and reaching `to`.
///
/// If `from` and `to` are equal, this function returns true.
///
/// If `space` is not `None`, it is used instead of creating a new workspace for
/// graph traversal.
pub fn has_path_connecting<G>(g: G, from: G::NodeId, to: G::NodeId,
                              space: Option<&mut DfsSpace<G::NodeId, G::Map>>)
    -> bool
    where G: IntoNeighbors + Visitable,
{
    with_dfs(g, space, |dfs| {
        dfs.reset(g);
        dfs.move_to(from);
        while let Some(x) = dfs.next(g) {
            if x == to {
                return true;
            }
        }
        false
    })
}

/// Renamed to `kosaraju_scc`.
#[deprecated(note = "renamed to kosaraju_scc")]
pub fn scc<G>(g: G) -> Vec<Vec<G::NodeId>>
    where G: IntoNeighborsDirected + Visitable + IntoNodeIdentifiers,
{
    kosaraju_scc(g)
}

/// [Generic] Compute the *strongly connected components* using [Kosaraju's algorithm][1].
///
/// [1]: https://en.wikipedia.org/wiki/Kosaraju%27s_algorithm
///
/// Return a vector where each element is a strongly connected component (scc).
/// The order of node ids within each scc is arbitrary, but the order of
/// the sccs is their postorder (reverse topological sort).
///
/// For an undirected graph, the sccs are simply the connected components.
///
/// This implementation is iterative and does two passes over the nodes.
pub fn kosaraju_scc<G>(g: G) -> Vec<Vec<G::NodeId>>
    where G: IntoNeighborsDirected + Visitable + IntoNodeIdentifiers,
{
    let mut dfs = DfsPostOrder::empty(g);

    // First phase, reverse dfs pass, compute finishing times.
    // http://stackoverflow.com/a/26780899/161659
    let mut finish_order = Vec::with_capacity(0);
    for i in g.node_identifiers() {
        if dfs.discovered.is_visited(&i) {
            continue
        }

        dfs.move_to(i);
        while let Some(nx) = dfs.next(Reversed(g)) {
            finish_order.push(nx);
        }
    }

    let mut dfs = Dfs::from_parts(dfs.stack, dfs.discovered);
    dfs.reset(g);
    let mut sccs = Vec::new();

    // Second phase
    // Process in decreasing finishing time order
    for i in finish_order.into_iter().rev() {
        if dfs.discovered.is_visited(&i) {
            continue;
        }
        // Move to the leader node `i`.
        dfs.move_to(i);
        let mut scc = Vec::new();
        while let Some(nx) = dfs.next(g) {
            scc.push(nx);
        }
        sccs.push(scc);
    }
    sccs
}

/// [Generic] Compute the *strongly connected components* using [Tarjan's algorithm][1].
///
/// [1]: https://en.wikipedia.org/wiki/Tarjan%27s_strongly_connected_components_algorithm
///
/// Return a vector where each element is a strongly connected component (scc).
/// The order of node ids within each scc is arbitrary, but the order of
/// the sccs is their postorder (reverse topological sort).
///
/// For an undirected graph, the sccs are simply the connected components.
///
/// This implementation is recursive and does one pass over the nodes.
pub fn tarjan_scc<G>(g: G) -> Vec<Vec<G::NodeId>>
    where G: IntoNodeIdentifiers + IntoNeighbors + NodeIndexable
{
    #[derive(Copy, Clone)]
    #[derive(Debug)]
    struct NodeData {
        index: Option<usize>,
        lowlink: usize,
        on_stack: bool,
    }

    #[derive(Debug)]
    struct Data<'a, G>
        where G: NodeIndexable, 
          G::NodeId: 'a
    {
        index: usize,
        nodes: Vec<NodeData>,
        stack: Vec<G::NodeId>,
        sccs: &'a mut Vec<Vec<G::NodeId>>,
    }

    fn scc_visit<G>(v: G::NodeId, g: G, data: &mut Data<G>) 
        where G: IntoNeighbors + NodeIndexable
    {
        macro_rules! node {
            ($node:expr) => (data.nodes[g.to_index($node)])
        }

        if node![v].index.is_some() {
            // already visited
            return;
        }

        let v_index = data.index;
        node![v].index = Some(v_index);
        node![v].lowlink = v_index;
        node![v].on_stack = true;
        data.stack.push(v);
        data.index += 1;

        for w in g.neighbors(v) {
            match node![w].index {
                None => {
                    scc_visit(w, g, data);
                    node![v].lowlink = min(node![v].lowlink, node![w].lowlink);
                }
                Some(w_index) => {
                    if node![w].on_stack {
                        // Successor w is in stack S and hence in the current SCC
                        let v_lowlink = &mut node![v].lowlink;
                        *v_lowlink = min(*v_lowlink, w_index);
                    }
                }
            }
        }

        // If v is a root node, pop the stack and generate an SCC
        if let Some(v_index) = node![v].index {
            if node![v].lowlink == v_index {
                let mut cur_scc = Vec::new();
                loop {
                    let w = data.stack.pop().unwrap();
                    node![w].on_stack = false;
                    cur_scc.push(w);
                    if g.to_index(w) == g.to_index(v) { break; }
                }
                data.sccs.push(cur_scc);
            }
        }
    }

    let mut sccs = Vec::new();
    {
        let map = vec![NodeData { index: None, lowlink: !0, on_stack: false }; g.node_bound()];

        let mut data = Data {
            index: 0,
            nodes: map,
            stack: Vec::new(),
            sccs: &mut sccs,
        };

        for n in g.node_identifiers() {
            scc_visit(n, g, &mut data);
        }
    }
    sccs
}

/// [Graph] Condense every strongly connected component into a single node and return the result.
///
/// If `make_acyclic` is true, self-loops and multi edges are ignored, guaranteeing that
/// the output is acyclic.
pub fn condensation<N, E, Ty, Ix>(g: Graph<N, E, Ty, Ix>, make_acyclic: bool) -> Graph<Vec<N>, E, Ty, Ix>
    where Ty: EdgeType,
          Ix: IndexType,
{
    let sccs = kosaraju_scc(&g);
    let mut condensed: Graph<Vec<N>, E, Ty, Ix> = Graph::with_capacity(sccs.len(), g.edge_count());

    // Build a map from old indices to new ones.
    let mut node_map = vec![NodeIndex::end(); g.node_count()];
    for comp in sccs {
        let new_nix = condensed.add_node(Vec::new());
        for nix in comp {
            node_map[nix.index()] = new_nix;
        }
    }

    // Consume nodes and edges of the old graph and insert them into the new one.
    let (nodes, edges) = g.into_nodes_edges();
    for (nix, node) in nodes.into_iter().enumerate() {
        condensed[node_map[nix]].push(node.weight);
    }
    for edge in edges {
        let source = node_map[edge.source().index()];
        let target = node_map[edge.target().index()];
        if make_acyclic {
            if source != target {
                condensed.update_edge(source, target, edge.weight);
            }
        } else {
            condensed.add_edge(source, target, edge.weight);
        }
    }
    condensed
}

/// [Generic] Compute a *minimum spanning tree* of a graph.
///
/// The input graph is treated as if undirected.
///
/// Using Kruskal's algorithm with runtime **O(|E| log |E|)**. We actually
/// return a minimum spanning forest, i.e. a minimum spanning tree for each connected
/// component of the graph.
///
/// The resulting graph has all the vertices of the input graph (with identical node indices),
/// and **|V| - c** edges, where **c** is the number of connected components in `g`.
///
/// Use `from_elements` to create a graph from the resulting iterator.
pub fn min_spanning_tree<G>(g: G) -> MinSpanningTree<G>
    where G::NodeWeight: Clone,
          G::EdgeWeight: Clone + PartialOrd,
          G: IntoNodeReferences + IntoEdgeReferences + NodeIndexable,
{

    // Initially each vertex is its own disjoint subgraph, track the connectedness
    // of the pre-MST with a union & find datastructure.
    let subgraphs = UnionFind::new(g.node_bound());

    let edges = g.edge_references();
    let mut sort_edges = BinaryHeap::with_capacity(edges.size_hint().0);
    for edge in edges {
        sort_edges.push(MinScored(edge.weight().clone(), (edge.source(), edge.target())));
    }

    MinSpanningTree {
        graph: g,
        node_ids: Some(g.node_references()),
        subgraphs: subgraphs,
        sort_edges: sort_edges,
    }

}

/// An iterator producing a minimum spanning forest of a graph.
pub struct MinSpanningTree<G>
    where G: Data + IntoNodeReferences,
{
    graph: G,
    node_ids: Option<G::NodeReferences>,
    subgraphs: UnionFind<usize>,
    sort_edges: BinaryHeap<MinScored<G::EdgeWeight, (G::NodeId, G::NodeId)>>,
}


impl<G> Iterator for MinSpanningTree<G>
    where G: IntoNodeReferences + NodeIndexable,
          G::NodeWeight: Clone,
          G::EdgeWeight: PartialOrd,
{
    type Item = Element<G::NodeWeight, G::EdgeWeight>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut iter) = self.node_ids {
            if let Some(node) = iter.next() {
                return Some(Element::Node { weight: node.weight().clone() });
            }
        }
        self.node_ids = None;

        // Kruskal's algorithm.
        // Algorithm is this:
        //
        // 1. Create a pre-MST with all the vertices and no edges.
        // 2. Repeat:
        //
        //  a. Remove the shortest edge from the original graph.
        //  b. If the edge connects two disjoint trees in the pre-MST,
        //     add the edge.
        while let Some(MinScored(score, (a, b))) = self.sort_edges.pop() {
            let g = self.graph;
            // check if the edge would connect two disjoint parts
            if self.subgraphs.union(g.to_index(a), g.to_index(b)) {
                return Some(Element::Edge {
                    source: g.to_index(a),
                    target: g.to_index(b),
                    weight: score,
                });
            }
        }
        None
    }
}

/// An algorithm error: a cycle was found in the graph.
#[derive(Clone, Debug, PartialEq)]
pub struct Cycle<N>(N);

impl<N> Cycle<N> {
    /// Return a node id that participates in the cycle
    pub fn node_id(&self) -> N
        where N: Copy
    {
        self.0
    }
}
/// An algorithm error: a cycle of negative weights was found in the graph.
#[derive(Clone, Debug, PartialEq)]
pub struct NegativeCycle(());

/// [Generic] Compute shortest paths from node `source` to all other.
///
/// Using the [Bellman–Ford algorithm][bf]; negative edge costs are
/// permitted, but the graph must not have a cycle of negative weights
/// (in that case it will return an error).
///
/// On success, return one vec with path costs, and another one which points
/// out the predecessor of a node along a shortest path. The vectors
/// are indexed by the graph's node indices.
///
/// [bf]: https://en.wikipedia.org/wiki/Bellman%E2%80%93Ford_algorithm
pub fn bellman_ford<G>(g: G, source: G::NodeId)
    -> Result<(Vec<G::EdgeWeight>, Vec<Option<G::NodeId>>), NegativeCycle>
    where G: NodeCount + IntoNodeIdentifiers + IntoEdges + NodeIndexable,
          G::EdgeWeight: FloatMeasure,
{
    let mut predecessor = vec![None; g.node_bound()];
    let mut distance = vec![<_>::infinite(); g.node_bound()];

    let ix = |i| g.to_index(i);

    distance[ix(source)] = <_>::zero();
    // scan up to |V| - 1 times.
    for _ in 1..g.node_count() {
        let mut did_update = false;
        for i in g.node_identifiers() {
            for edge in g.edges(i) {
                let i = edge.source();
                let j = edge.target();
                let w = *edge.weight();
                if distance[ix(i)] + w < distance[ix(j)] {
                    distance[ix(j)] = distance[ix(i)] + w;
                    predecessor[ix(j)] = Some(i);
                    did_update = true;
                }
            }
        }
        if !did_update {
            break;
        }
    }

    // check for negative weight cycle
    for i in g.node_identifiers() {
        for edge in g.edges(i) {
            let j = edge.target();
            let w = *edge.weight();
            if distance[ix(i)] + w < distance[ix(j)] {
                //println!("neg cycle, detected from {} to {}, weight={}", i, j, w);
                return Err(NegativeCycle(()));
            }
        }
    }

    Ok((distance, predecessor))
}

use std::ops::Add;
use std::fmt::Debug;

/// Associated data that can be used for measures (such as length).
pub trait Measure : Debug + PartialOrd + Add<Self, Output=Self> + Default + Clone {
}

impl<M> Measure for M
    where M: Debug + PartialOrd + Add<M, Output=M> + Default + Clone,
{ }

/// A floating-point measure.
pub trait FloatMeasure : Measure + Copy {
    fn zero() -> Self;
    fn infinite() -> Self;
}

impl FloatMeasure for f32 {
    fn zero() -> Self { 0. }
    fn infinite() -> Self { 1./0. }
}

impl FloatMeasure for f64 {
    fn zero() -> Self { 0. }
    fn infinite() -> Self { 1./0. }
}

