

use visit::IntoNeighbors;
use visit::{VisitMap, Visitable};

/// Strictly monotonically increasing event time for a depth first search.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Default, Hash)]
pub struct Time(pub usize);

/// A depth first search (DFS) visitor event.
#[derive(Copy, Clone, Debug)]
pub enum DfsEvent<N> {
    Discover(N, Time),
    /// An edge of the tree formed by the traversal.
    TreeEdge(N, N),
    /// An edge to an already visited node.
    BackEdge(N, N),
    /// A cross or forward edge.
    ///
    /// For an edge *(u, v)*, if the discover time of *v* is greater than *u*,
    /// then it is a forward edge, else a cross edge.
    CrossForwardEdge(N, N),
    Finish(N, Time),
}

/// Return if the expression is a break value.
macro_rules! try_control {
    ($e:expr) => {
        match $e {
            x => if x.should_break() {
                return x;
            }
        }
    }
}

/// Control flow for callbacks.
///
/// `Break` can carry a value.
#[derive(Copy, Clone, Debug)]
pub enum Control<B> {
    Continue,
    Break(B),
}

impl<B> Control<B> {
    pub fn breaking() -> Control<()> { Control::Break(()) }
    /// Get the value in `Control::Break(_)`, if present.
    pub fn break_value(self) -> Option<B> {
        match self {
            Control::Continue => None,
            Control::Break(b) => Some(b),
        }
    }
}

/// Control flow for callbacks.
///
/// The empty return value `()` is equivalent to continue.
pub trait ControlFlow {
    fn continuing() -> Self;
    fn should_break(&self) -> bool;
}

impl ControlFlow for () {
    fn continuing() { }
    #[inline]
    fn should_break(&self) -> bool { false }
}

impl<B> ControlFlow for Control<B> {
    fn continuing() -> Self { Control::Continue }
    fn should_break(&self) -> bool {
        if let Control::Break(_) = *self { true } else { false }
    }
}

impl<E> ControlFlow for Result<(), E> {
    fn continuing() -> Self { Ok(()) }
    fn should_break(&self) -> bool {
        self.is_err()
    }
}

/// The default is `Continue`.
impl<B> Default for Control<B> {
    fn default() -> Self { Control::Continue }
}

/// A recursive depth first search.
///
/// Starting points are the nodes in the iterator `starts` (specify just one
/// start vertex *x* by using `Some(x)`).
///
/// The traversal emits discovery and finish events for each reachable vertex,
/// and edge classification of each reachable edge. `visitor` is called for each
/// event, see [`DfsEvent`][de] for possible values.
///
/// If the return value of the visitor is simply `()`, the visit runs until it
/// is finished. If the return value is a `Control<B>`, it can be used to
/// break the visit early, and the last control value is returned by the
/// function.
///
/// [de]: enum.DfsEvent.html
///
/// # Example
///
/// Find a path from vertex 0 to 5, and exit the visit as soon as we reach
/// the goal vertex.
///
/// ```
/// use petgraph::prelude::*;
/// use petgraph::graph::node_index as n;
/// use petgraph::visit::depth_first_search;
/// use petgraph::visit::{DfsEvent, Control};
///
/// let gr: Graph<(), ()> = Graph::from_edges(&[
///     (0, 1), (0, 2), (0, 3),
///     (1, 3),
///     (2, 3), (2, 4),
///     (4, 0), (4, 5),
/// ]);
///
/// // record each predecessor, mapping node → node
/// let mut predecessor = vec![NodeIndex::end(); gr.node_count()];
/// let start = n(0);
/// let goal = n(5);
/// depth_first_search(&gr, Some(start), |event| {
///     if let DfsEvent::TreeEdge(u, v) = event {
///         predecessor[v.index()] = u;
///         if v == goal {
///             return Control::Break(v);
///         }
///     }
///     Control::Continue
/// });
///
/// let mut next = goal;
/// let mut path = vec![next];
/// while next != start {
///     let pred = predecessor[next.index()];
///     path.push(pred);
///     next = pred;
/// }
/// path.reverse();
/// assert_eq!(&path, &[n(0), n(2), n(4), n(5)]);
/// ```
pub fn depth_first_search<G, I, F, C>(graph: G, starts: I, mut visitor: F) -> C
    where G: IntoNeighbors + Visitable,
          I: IntoIterator<Item=G::NodeId>,
          F: FnMut(DfsEvent<G::NodeId>) -> C,
          C: ControlFlow,
{
    let time = &mut Time(0);
    let discovered = &mut graph.visit_map();
    let finished = &mut graph.visit_map();

    for start in starts {
        try_control!(dfs_visitor(graph, start, &mut visitor, discovered, finished, time));
    }
    C::continuing()
}

fn dfs_visitor<G, F, C>(graph: G, u: G::NodeId, visitor: &mut F,
                     discovered: &mut G::Map, finished: &mut G::Map,
                     time: &mut Time) -> C
    where G: IntoNeighbors + Visitable,
          F: FnMut(DfsEvent<G::NodeId>) -> C,
          C: ControlFlow,
{
    if !discovered.visit(u) {
        return C::continuing();
    }
    try_control!(visitor(DfsEvent::Discover(u, time_post_inc(time))));
    for v in graph.neighbors(u) {
        if !discovered.is_visited(&v) {
            try_control!(visitor(DfsEvent::TreeEdge(u, v)));
            try_control!(dfs_visitor(graph, v, visitor, discovered, finished, time));
        } else if !finished.is_visited(&v) {
            try_control!(visitor(DfsEvent::BackEdge(u, v)));
        } else {
            try_control!(visitor(DfsEvent::CrossForwardEdge(u, v)));
        }
    }
    let first_finish = finished.visit(u);
    debug_assert!(first_finish);
    try_control!(visitor(DfsEvent::Finish(u, time_post_inc(time))));
    C::continuing()
}

fn time_post_inc(x: &mut Time) -> Time {
    let v = *x;
    x.0 += 1;
    v
}
