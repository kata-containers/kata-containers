use fixedbitset::FixedBitSet;
use std::marker;

use super::graph::{Graph, IndexType, NodeIndex};
use super::{EdgeType, Incoming};

use super::visit::GetAdjacencyMatrix;

#[derive(Debug)]
struct Vf2State<Ty, Ix> {
    /// The current mapping M(s) of nodes from G0 → G1 and G1 → G0,
    /// NodeIndex::end() for no mapping.
    mapping: Vec<NodeIndex<Ix>>,
    /// out[i] is non-zero if i is in either M_0(s) or Tout_0(s)
    /// These are all the next vertices that are not mapped yet, but
    /// have an outgoing edge from the mapping.
    out: Vec<usize>,
    /// ins[i] is non-zero if i is in either M_0(s) or Tin_0(s)
    /// These are all the incoming vertices, those not mapped yet, but
    /// have an edge from them into the mapping.
    /// Unused if graph is undirected -- it's identical with out in that case.
    ins: Vec<usize>,
    out_size: usize,
    ins_size: usize,
    adjacency_matrix: FixedBitSet,
    generation: usize,
    _etype: marker::PhantomData<Ty>,
}

impl<Ty, Ix> Vf2State<Ty, Ix>
where
    Ty: EdgeType,
    Ix: IndexType,
{
    pub fn new<N, E>(g: &Graph<N, E, Ty, Ix>) -> Self {
        let c0 = g.node_count();
        let mut state = Vf2State {
            mapping: Vec::with_capacity(c0),
            out: Vec::with_capacity(c0),
            ins: Vec::with_capacity(c0 * (g.is_directed() as usize)),
            out_size: 0,
            ins_size: 0,
            adjacency_matrix: g.adjacency_matrix(),
            generation: 0,
            _etype: marker::PhantomData,
        };
        for _ in 0..c0 {
            state.mapping.push(NodeIndex::end());
            state.out.push(0);
            if Ty::is_directed() {
                state.ins.push(0);
            }
        }
        state
    }

    /// Return **true** if we have a complete mapping
    pub fn is_complete(&self) -> bool {
        self.generation == self.mapping.len()
    }

    /// Add mapping **from** <-> **to** to the state.
    pub fn push_mapping<N, E>(
        &mut self,
        from: NodeIndex<Ix>,
        to: NodeIndex<Ix>,
        g: &Graph<N, E, Ty, Ix>,
    ) {
        self.generation += 1;
        let s = self.generation;
        self.mapping[from.index()] = to;
        // update T0 & T1 ins/outs
        // T0out: Node in G0 not in M0 but successor of a node in M0.
        // st.out[0]: Node either in M0 or successor of M0
        for ix in g.neighbors(from) {
            if self.out[ix.index()] == 0 {
                self.out[ix.index()] = s;
                self.out_size += 1;
            }
        }
        if g.is_directed() {
            for ix in g.neighbors_directed(from, Incoming) {
                if self.ins[ix.index()] == 0 {
                    self.ins[ix.index()] = s;
                    self.ins_size += 1;
                }
            }
        }
    }

    /// Restore the state to before the last added mapping
    pub fn pop_mapping<N, E>(&mut self, from: NodeIndex<Ix>, g: &Graph<N, E, Ty, Ix>) {
        let s = self.generation;
        self.generation -= 1;

        // undo (n, m) mapping
        self.mapping[from.index()] = NodeIndex::end();

        // unmark in ins and outs
        for ix in g.neighbors(from) {
            if self.out[ix.index()] == s {
                self.out[ix.index()] = 0;
                self.out_size -= 1;
            }
        }
        if g.is_directed() {
            for ix in g.neighbors_directed(from, Incoming) {
                if self.ins[ix.index()] == s {
                    self.ins[ix.index()] = 0;
                    self.ins_size -= 1;
                }
            }
        }
    }

    /// Find the next (least) node in the Tout set.
    pub fn next_out_index(&self, from_index: usize) -> Option<usize> {
        self.out[from_index..]
            .iter()
            .enumerate()
            .find(move |&(index, elt)| {
                *elt > 0 && self.mapping[from_index + index] == NodeIndex::end()
            })
            .map(|(index, _)| index)
    }

    /// Find the next (least) node in the Tin set.
    pub fn next_in_index(&self, from_index: usize) -> Option<usize> {
        if !Ty::is_directed() {
            return None;
        }
        self.ins[from_index..]
            .iter()
            .enumerate()
            .find(move |&(index, elt)| {
                *elt > 0 && self.mapping[from_index + index] == NodeIndex::end()
            })
            .map(|(index, _)| index)
    }

    /// Find the next (least) node in the N - M set.
    pub fn next_rest_index(&self, from_index: usize) -> Option<usize> {
        self.mapping[from_index..]
            .iter()
            .enumerate()
            .find(|&(_, elt)| *elt == NodeIndex::end())
            .map(|(index, _)| index)
    }
}

/// [Graph] Return `true` if the graphs `g0` and `g1` are isomorphic.
///
/// Using the VF2 algorithm, only matching graph syntactically (graph
/// structure).
///
/// The graphs should not be multigraphs.
///
/// **Reference**
///
/// * Luigi P. Cordella, Pasquale Foggia, Carlo Sansone, Mario Vento;
///   *A (Sub)Graph Isomorphism Algorithm for Matching Large Graphs*
pub fn is_isomorphic<N, E, Ty, Ix>(g0: &Graph<N, E, Ty, Ix>, g1: &Graph<N, E, Ty, Ix>) -> bool
where
    Ty: EdgeType,
    Ix: IndexType,
{
    if g0.node_count() != g1.node_count() || g0.edge_count() != g1.edge_count() {
        return false;
    }

    let mut st = [Vf2State::new(g0), Vf2State::new(g1)];
    try_match(&mut st, g0, g1, &mut NoSemanticMatch, &mut NoSemanticMatch).unwrap_or(false)
}

/// [Graph] Return `true` if the graphs `g0` and `g1` are isomorphic.
///
/// Using the VF2 algorithm, examining both syntactic and semantic
/// graph isomorphism (graph structure and matching node and edge weights).
///
/// The graphs should not be multigraphs.
pub fn is_isomorphic_matching<N, E, Ty, Ix, F, G>(
    g0: &Graph<N, E, Ty, Ix>,
    g1: &Graph<N, E, Ty, Ix>,
    mut node_match: F,
    mut edge_match: G,
) -> bool
where
    Ty: EdgeType,
    Ix: IndexType,
    F: FnMut(&N, &N) -> bool,
    G: FnMut(&E, &E) -> bool,
{
    if g0.node_count() != g1.node_count() || g0.edge_count() != g1.edge_count() {
        return false;
    }

    let mut st = [Vf2State::new(g0), Vf2State::new(g1)];
    try_match(&mut st, g0, g1, &mut node_match, &mut edge_match).unwrap_or(false)
}

trait SemanticMatcher<T> {
    fn enabled() -> bool;
    fn eq(&mut self, _: &T, _: &T) -> bool;
}

struct NoSemanticMatch;

impl<T> SemanticMatcher<T> for NoSemanticMatch {
    #[inline]
    fn enabled() -> bool {
        false
    }
    #[inline]
    fn eq(&mut self, _: &T, _: &T) -> bool {
        true
    }
}

impl<T, F> SemanticMatcher<T> for F
where
    F: FnMut(&T, &T) -> bool,
{
    #[inline]
    fn enabled() -> bool {
        true
    }
    #[inline]
    fn eq(&mut self, a: &T, b: &T) -> bool {
        self(a, b)
    }
}

/// Return Some(bool) if isomorphism is decided, else None.
fn try_match<N, E, Ty, Ix, F, G>(
    mut st: &mut [Vf2State<Ty, Ix>; 2],
    g0: &Graph<N, E, Ty, Ix>,
    g1: &Graph<N, E, Ty, Ix>,
    node_match: &mut F,
    edge_match: &mut G,
) -> Option<bool>
where
    Ty: EdgeType,
    Ix: IndexType,
    F: SemanticMatcher<N>,
    G: SemanticMatcher<E>,
{
    if st[0].is_complete() {
        return Some(true);
    }
    let g = [g0, g1];
    let graph_indices = 0..2;
    let end = NodeIndex::end();

    // A "depth first" search of a valid mapping from graph 1 to graph 2

    // F(s, n, m) -- evaluate state s and add mapping n <-> m

    // Find least T1out node (in st.out[1] but not in M[1])
    #[derive(Copy, Clone, PartialEq, Debug)]
    enum OpenList {
        Out,
        In,
        Other,
    }

    #[derive(Clone, PartialEq, Debug)]
    enum Frame<N: marker::Copy> {
        Outer,
        Inner { nodes: [N; 2], open_list: OpenList },
        Unwind { nodes: [N; 2], open_list: OpenList },
    }

    let next_candidate =
        |st: &mut [Vf2State<Ty, Ix>; 2]| -> Option<(NodeIndex<Ix>, NodeIndex<Ix>, OpenList)> {
            let mut to_index;
            let mut from_index = None;
            let mut open_list = OpenList::Out;
            // Try the out list
            to_index = st[1].next_out_index(0);

            if to_index.is_some() {
                from_index = st[0].next_out_index(0);
                open_list = OpenList::Out;
            }
            // Try the in list
            if to_index.is_none() || from_index.is_none() {
                to_index = st[1].next_in_index(0);

                if to_index.is_some() {
                    from_index = st[0].next_in_index(0);
                    open_list = OpenList::In;
                }
            }
            // Try the other list -- disconnected graph
            if to_index.is_none() || from_index.is_none() {
                to_index = st[1].next_rest_index(0);
                if to_index.is_some() {
                    from_index = st[0].next_rest_index(0);
                    open_list = OpenList::Other;
                }
            }
            match (from_index, to_index) {
                (Some(n), Some(m)) => Some((NodeIndex::new(n), NodeIndex::new(m), open_list)),
                // No more candidates
                _ => None,
            }
        };
    let next_from_ix = |st: &mut [Vf2State<Ty, Ix>; 2],
                        nx: NodeIndex<Ix>,
                        open_list: OpenList|
     -> Option<NodeIndex<Ix>> {
        // Find the next node index to try on the `from` side of the mapping
        let start = nx.index() + 1;
        let cand0 = match open_list {
            OpenList::Out => st[0].next_out_index(start),
            OpenList::In => st[0].next_in_index(start),
            OpenList::Other => st[0].next_rest_index(start),
        }
        .map(|c| c + start); // compensate for start offset.
        match cand0 {
            None => None, // no more candidates
            Some(ix) => {
                debug_assert!(ix >= start);
                Some(NodeIndex::new(ix))
            }
        }
    };
    //fn pop_state(nodes: [NodeIndex<Ix>; 2]) {
    let pop_state = |st: &mut [Vf2State<Ty, Ix>; 2], nodes: [NodeIndex<Ix>; 2]| {
        // Restore state.
        for j in graph_indices.clone() {
            st[j].pop_mapping(nodes[j], g[j]);
        }
    };
    //fn push_state(nodes: [NodeIndex<Ix>; 2]) {
    let push_state = |st: &mut [Vf2State<Ty, Ix>; 2], nodes: [NodeIndex<Ix>; 2]| {
        // Add mapping nx <-> mx to the state
        for j in graph_indices.clone() {
            st[j].push_mapping(nodes[j], nodes[1 - j], g[j]);
        }
    };
    //fn is_feasible(nodes: [NodeIndex<Ix>; 2]) -> bool {
    let mut is_feasible = |st: &mut [Vf2State<Ty, Ix>; 2], nodes: [NodeIndex<Ix>; 2]| -> bool {
        // Check syntactic feasibility of mapping by ensuring adjacencies
        // of nx map to adjacencies of mx.
        //
        // nx == map to => mx
        //
        // R_succ
        //
        // Check that every neighbor of nx is mapped to a neighbor of mx,
        // then check the reverse, from mx to nx. Check that they have the same
        // count of edges.
        //
        // Note: We want to check the lookahead measures here if we can,
        // R_out: Equal for G0, G1: Card(Succ(G, n) ^ Tout); for both Succ and Pred
        // R_in: Same with Tin
        // R_new: Equal for G0, G1: Ñ n Pred(G, n); both Succ and Pred,
        //      Ñ is G0 - M - Tin - Tout
        // last attempt to add these did not speed up any of the testcases
        let mut succ_count = [0, 0];
        for j in graph_indices.clone() {
            for n_neigh in g[j].neighbors(nodes[j]) {
                succ_count[j] += 1;
                // handle the self loop case; it's not in the mapping (yet)
                let m_neigh = if nodes[j] != n_neigh {
                    st[j].mapping[n_neigh.index()]
                } else {
                    nodes[1 - j]
                };
                if m_neigh == end {
                    continue;
                }
                let has_edge =
                    g[1 - j].is_adjacent(&st[1 - j].adjacency_matrix, nodes[1 - j], m_neigh);
                if !has_edge {
                    return false;
                }
            }
        }
        if succ_count[0] != succ_count[1] {
            return false;
        }
        // R_pred
        if g[0].is_directed() {
            let mut pred_count = [0, 0];
            for j in graph_indices.clone() {
                for n_neigh in g[j].neighbors_directed(nodes[j], Incoming) {
                    pred_count[j] += 1;
                    // the self loop case is handled in outgoing
                    let m_neigh = st[j].mapping[n_neigh.index()];
                    if m_neigh == end {
                        continue;
                    }
                    let has_edge =
                        g[1 - j].is_adjacent(&st[1 - j].adjacency_matrix, m_neigh, nodes[1 - j]);
                    if !has_edge {
                        return false;
                    }
                }
            }
            if pred_count[0] != pred_count[1] {
                return false;
            }
        }
        // semantic feasibility: compare associated data for nodes
        if F::enabled() && !node_match.eq(&g[0][nodes[0]], &g[1][nodes[1]]) {
            return false;
        }
        // semantic feasibility: compare associated data for edges
        if G::enabled() {
            // outgoing edges
            for j in graph_indices.clone() {
                let mut edges = g[j].neighbors(nodes[j]).detach();
                while let Some((n_edge, n_neigh)) = edges.next(g[j]) {
                    // handle the self loop case; it's not in the mapping (yet)
                    let m_neigh = if nodes[j] != n_neigh {
                        st[j].mapping[n_neigh.index()]
                    } else {
                        nodes[1 - j]
                    };
                    if m_neigh == end {
                        continue;
                    }
                    match g[1 - j].find_edge(nodes[1 - j], m_neigh) {
                        Some(m_edge) => {
                            if !edge_match.eq(&g[j][n_edge], &g[1 - j][m_edge]) {
                                return false;
                            }
                        }
                        None => unreachable!(), // covered by syntactic check
                    }
                }
            }
            // incoming edges
            if g[0].is_directed() {
                for j in graph_indices.clone() {
                    let mut edges = g[j].neighbors_directed(nodes[j], Incoming).detach();
                    while let Some((n_edge, n_neigh)) = edges.next(g[j]) {
                        // the self loop case is handled in outgoing
                        let m_neigh = st[j].mapping[n_neigh.index()];
                        if m_neigh == end {
                            continue;
                        }
                        match g[1 - j].find_edge(m_neigh, nodes[1 - j]) {
                            Some(m_edge) => {
                                if !edge_match.eq(&g[j][n_edge], &g[1 - j][m_edge]) {
                                    return false;
                                }
                            }
                            None => unreachable!(), // covered by syntactic check
                        }
                    }
                }
            }
        }
        true
    };
    let mut stack: Vec<Frame<NodeIndex<Ix>>> = vec![Frame::Outer];

    while let Some(frame) = stack.pop() {
        match frame {
            Frame::Unwind {
                nodes,
                open_list: ol,
            } => {
                pop_state(&mut st, nodes);

                match next_from_ix(&mut st, nodes[0], ol) {
                    None => continue,
                    Some(nx) => {
                        let f = Frame::Inner {
                            nodes: [nx, nodes[1]],
                            open_list: ol,
                        };
                        stack.push(f);
                    }
                }
            }
            Frame::Outer => match next_candidate(&mut st) {
                None => continue,
                Some((nx, mx, ol)) => {
                    let f = Frame::Inner {
                        nodes: [nx, mx],
                        open_list: ol,
                    };
                    stack.push(f);
                }
            },
            Frame::Inner {
                nodes,
                open_list: ol,
            } => {
                if is_feasible(&mut st, nodes) {
                    push_state(&mut st, nodes);
                    if st[0].is_complete() {
                        return Some(true);
                    }
                    // Check cardinalities of Tin, Tout sets
                    if st[0].out_size == st[1].out_size && st[0].ins_size == st[1].ins_size {
                        let f0 = Frame::Unwind {
                            nodes,
                            open_list: ol,
                        };
                        stack.push(f0);
                        stack.push(Frame::Outer);
                        continue;
                    }
                    pop_state(&mut st, nodes);
                }
                match next_from_ix(&mut st, nodes[0], ol) {
                    None => continue,
                    Some(nx) => {
                        let f = Frame::Inner {
                            nodes: [nx, nodes[1]],
                            open_list: ol,
                        };
                        stack.push(f);
                    }
                }
            }
        }
    }
    None
}
