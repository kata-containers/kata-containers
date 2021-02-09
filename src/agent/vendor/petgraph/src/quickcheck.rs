extern crate quickcheck;

use self::quickcheck::{Gen, Arbitrary};

use {
    Graph,
    EdgeType,
};
use graph::{
    IndexType,
    node_index,
};
#[cfg(feature = "stable_graph")]
use stable_graph::StableGraph;

#[cfg(feature = "graphmap")]
use graphmap::{
    GraphMap,
    NodeTrait,
};
use visit::NodeIndexable;

/// `Arbitrary` for `Graph` creates a graph by selecting a node count
/// and a probability for each possible edge to exist.
///
/// The result will be simple graph or digraph, self loops
/// possible, no parallel edges.
///
/// The exact properties of the produced graph is subject to change.
///
/// Requires crate feature `"quickcheck"`
impl<N, E, Ty, Ix> Arbitrary for Graph<N, E, Ty, Ix>
    where N: Arbitrary,
          E: Arbitrary,
          Ty: EdgeType + Send + 'static,
          Ix: IndexType + Send,
{
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let nodes = usize::arbitrary(g);
        if nodes == 0 {
            return Graph::with_capacity(0, 0);
        }
        // use X² for edge probability (bias towards lower)
        let edge_prob = g.gen_range(0., 1.) * g.gen_range(0., 1.);
        let edges = ((nodes as f64).powi(2) * edge_prob) as usize;
        let mut gr = Graph::with_capacity(nodes, edges);
        for _ in 0..nodes {
            gr.add_node(N::arbitrary(g));
        }
        for i in gr.node_indices() {
            for j in gr.node_indices() {
                if !gr.is_directed() && i > j {
                    continue;
                }
                let p: f64 = g.gen();
                if p <= edge_prob {
                    gr.add_edge(i, j, E::arbitrary(g));
                }
            }
        }
        gr
    }

    // shrink the graph by splitting it in two by a very
    // simple algorithm, just even and odd node indices
    fn shrink(&self) -> Box<Iterator<Item=Self>> {
        let self_ = self.clone();
        Box::new((0..2).filter_map(move |x| {
            let gr = self_.filter_map(|i, w| {
                if i.index() % 2 == x {
                    Some(w.clone())
                } else {
                    None
                }
            },
            |_, w| Some(w.clone())
            );
            // make sure we shrink
            if gr.node_count() < self_.node_count() {
                Some(gr)
            } else {
                None
            }
        }))
    }
}

#[cfg(feature = "stable_graph")]
/// `Arbitrary` for `StableGraph` creates a graph by selecting a node count
/// and a probability for each possible edge to exist.
///
/// The result will be simple graph or digraph, with possible
/// self loops, no parallel edges.
///
/// The exact properties of the produced graph is subject to change.
///
/// Requires crate features `"quickcheck"` and `"stable_graph"`
impl<N, E, Ty, Ix> Arbitrary for StableGraph<N, E, Ty, Ix>
    where N: Arbitrary,
          E: Arbitrary,
          Ty: EdgeType + Send + 'static,
          Ix: IndexType + Send,
{
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let nodes = usize::arbitrary(g);
        if nodes == 0 {
            return StableGraph::with_capacity(0, 0);
        }
        // use X² for edge probability (bias towards lower)
        let edge_prob = g.gen_range(0., 1.) * g.gen_range(0., 1.);
        let edges = ((nodes as f64).powi(2) * edge_prob) as usize;
        let mut gr = StableGraph::with_capacity(nodes, edges);
        for _ in 0..nodes {
            gr.add_node(N::arbitrary(g));
        }
        for i in 0..gr.node_count() {
            for j in 0..gr.node_count() {
                let i = node_index(i);
                let j = node_index(j);
                if !gr.is_directed() && i > j {
                    continue;
                }
                let p: f64 = g.gen();
                if p <= edge_prob {
                    gr.add_edge(i, j, E::arbitrary(g));
                }
            }
        }
        if bool::arbitrary(g) {
            // potentially remove nodes to make holes in nodes & edge sets
            let n = u8::arbitrary(g) % (gr.node_count() as u8);
            for _ in 0..n {
                let ni = node_index(usize::arbitrary(g) % gr.node_bound());
                if gr.node_weight(ni).is_some() {
                    gr.remove_node(ni);
                }
            }
        }
        gr
    }

    // shrink the graph by splitting it in two by a very
    // simple algorithm, just even and odd node indices
    fn shrink(&self) -> Box<Iterator<Item=Self>> {
        let self_ = self.clone();
        Box::new((0..2).filter_map(move |x| {
            let gr = self_.filter_map(|i, w| {
                if i.index() % 2 == x {
                    Some(w.clone())
                } else {
                    None
                }
            },
            |_, w| Some(w.clone())
            );
            // make sure we shrink
            if gr.node_count() < self_.node_count() {
                Some(gr)
            } else {
                None
            }
        }))
    }
}

/// `Arbitrary` for `GraphMap` creates a graph by selecting a node count
/// and a probability for each possible edge to exist.
///
/// The result will be simple graph or digraph, self loops
/// possible, no parallel edges.
///
/// The exact properties of the produced graph is subject to change.
///
/// Requires crate features `"quickcheck"` and `"graphmap"`
#[cfg(feature = "graphmap")]
impl<N, E, Ty> Arbitrary for GraphMap<N, E, Ty>
    where N: NodeTrait + Arbitrary,
          E: Arbitrary,
          Ty: EdgeType + Clone + Send + 'static,
{
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let nodes = usize::arbitrary(g);
        if nodes == 0 {
            return GraphMap::with_capacity(0, 0);
        }
        let mut nodes = (0..nodes).map(|_| N::arbitrary(g)).collect::<Vec<_>>();
        nodes.sort();
        nodes.dedup();

        // use X² for edge probability (bias towards lower)
        let edge_prob = g.gen_range(0., 1.) * g.gen_range(0., 1.);
        let edges = ((nodes.len() as f64).powi(2) * edge_prob) as usize;
        let mut gr = GraphMap::with_capacity(nodes.len(), edges);
        for &node in &nodes {
            gr.add_node(node);
        }
        for (index, &i) in nodes.iter().enumerate() {
            let js = if Ty::is_directed() { &nodes[..] } else { &nodes[index..] };
            for &j in js {
                let p: f64 = g.gen();
                if p <= edge_prob {
                    gr.add_edge(i, j, E::arbitrary(g));
                }
            }
        }
        gr
    }
}
