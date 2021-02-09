
use prelude::*;

use fixedbitset::FixedBitSet;
use std::collections::HashSet;
use std::marker::PhantomData;

use visit::{
    GraphBase,
    GraphProp,
    IntoEdgeReferences,
    IntoEdges,
    IntoNeighbors,
    IntoNeighborsDirected,
    IntoNodeIdentifiers,
    IntoNodeReferences,
    NodeIndexable,
    NodeRef,
    VisitMap,
    Visitable,
};
use visit::{Data, NodeCompactIndexable, NodeCount};
use data::{DataMap};

/// A graph filter for nodes.
pub trait FilterNode<N>
{
    /// Return true to have the node be part of the graph
    fn include_node(&self, node: N) -> bool;
}

impl<F, N> FilterNode<N> for F
    where F: Fn(N) -> bool,
{
    fn include_node(&self, n: N) -> bool {
        (*self)(n)
    }
}

/// This filter includes the nodes that are contained in the set.
impl<N> FilterNode<N> for FixedBitSet
    where FixedBitSet: VisitMap<N>,
{
    fn include_node(&self, n: N) -> bool {
        self.is_visited(&n)
    }
}

/// This filter includes the nodes that are contained in the set.
impl<N, S> FilterNode<N> for HashSet<N, S>
    where HashSet<N, S>: VisitMap<N>,
{
    fn include_node(&self, n: N) -> bool {
        self.is_visited(&n)
    }
}

/// A node-filtering graph adaptor.
#[derive(Copy, Clone, Debug)]
pub struct NodeFiltered<G, F>(pub G, pub F);

impl<F, G> NodeFiltered<G, F>
    where G: GraphBase,
          F: Fn(G::NodeId) -> bool,
{
    /// Create an `NodeFiltered` adaptor from the closure `filter`.
    pub fn from_fn(graph: G, filter: F) -> Self {
        NodeFiltered(graph, filter)
    }
}

impl<G, F> GraphBase for NodeFiltered<G, F> where G: GraphBase {
    type NodeId = G::NodeId;
    type EdgeId = G::EdgeId;
}

impl<'a, G, F> IntoNeighbors for &'a NodeFiltered<G, F>
    where G: IntoNeighbors,
          F: FilterNode<G::NodeId>,
{
    type Neighbors = NodeFilteredNeighbors<'a, G::Neighbors, F>;
    fn neighbors(self, n: G::NodeId) -> Self::Neighbors {
        NodeFilteredNeighbors {
            include_source: self.1.include_node(n),
            iter: self.0.neighbors(n),
            f: &self.1,
        }
    }
}

/// A filtered neighbors iterator.
pub struct NodeFilteredNeighbors<'a, I, F: 'a>
{
    include_source: bool,
    iter: I,
    f: &'a F,
}

impl<'a, I, F> Iterator for NodeFilteredNeighbors<'a, I, F>
    where I: Iterator,
          I::Item: Copy,
          F: FilterNode<I::Item>,
{
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        let f = self.f;
        if !self.include_source {
            None
        } else {
            self.iter.find(move |&target| f.include_node(target))
        }
    }
}

impl<'a, G, F> IntoNeighborsDirected for &'a NodeFiltered<G, F>
    where G: IntoNeighborsDirected,
          F: FilterNode<G::NodeId>,
{
    type NeighborsDirected = NodeFilteredNeighbors<'a, G::NeighborsDirected, F>;
    fn neighbors_directed(self, n: G::NodeId, dir: Direction)
        -> Self::NeighborsDirected {
        NodeFilteredNeighbors {
            include_source: self.1.include_node(n),
            iter: self.0.neighbors_directed(n, dir),
            f: &self.1,
        }
    }
}

impl<'a, G, F> IntoNodeIdentifiers for &'a NodeFiltered<G, F>
    where G: IntoNodeIdentifiers,
          F: FilterNode<G::NodeId>,
{
    type NodeIdentifiers = NodeFilteredNeighbors<'a, G::NodeIdentifiers, F>;
    fn node_identifiers(self) -> Self::NodeIdentifiers {
        NodeFilteredNeighbors {
            include_source: true,
            iter: self.0.node_identifiers(),
            f: &self.1,
        }
    }
}

impl<'a, G, F> IntoNodeReferences for &'a NodeFiltered<G, F>
    where G: IntoNodeReferences,
          F: FilterNode<G::NodeId>,
{
    type NodeRef = G::NodeRef;
    type NodeReferences = NodeFilteredNodes<'a, G::NodeReferences, F>;
    fn node_references(self) -> Self::NodeReferences {
        NodeFilteredNodes {
            include_source: true,
            iter: self.0.node_references(),
            f: &self.1,
        }
    }
}

/// A filtered node references iterator.
pub struct NodeFilteredNodes<'a, I, F: 'a>
{
    include_source: bool,
    iter: I,
    f: &'a F,
}

impl<'a, I, F> Iterator for NodeFilteredNodes<'a, I, F>
    where I: Iterator,
          I::Item: Copy + NodeRef,
          F: FilterNode<<I::Item as NodeRef>::NodeId>,
{
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        let f = self.f;
        if !self.include_source {
            None
        } else {
            self.iter.find(move |&target| f.include_node(target.id()))
        }
    }
}

impl<'a, G, F> IntoEdgeReferences for &'a NodeFiltered<G, F>
    where G: IntoEdgeReferences,
          F: FilterNode<G::NodeId>,
{
    type EdgeRef = G::EdgeRef;
    type EdgeReferences = NodeFilteredEdgeReferences<'a, G, G::EdgeReferences, F>;
    fn edge_references(self) -> Self::EdgeReferences {
        NodeFilteredEdgeReferences {
            graph: PhantomData,
            iter: self.0.edge_references(),
            f: &self.1,
        }
    }
}

/// A filtered edges iterator.
pub struct NodeFilteredEdgeReferences<'a, G, I, F: 'a>
{
    graph: PhantomData<G>,
    iter: I,
    f: &'a F,
}

impl<'a, G, I, F> Iterator for NodeFilteredEdgeReferences<'a, G, I, F>
    where F: FilterNode<G::NodeId>,
          G: IntoEdgeReferences,
          I: Iterator<Item=G::EdgeRef>,
{
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        let f = self.f;
        self.iter.find(move |&edge| f.include_node(edge.source()) &&
                                    f.include_node(edge.target()))
    }
}

impl<'a, G, F> IntoEdges for &'a NodeFiltered<G, F>
    where G: IntoEdges,
          F: FilterNode<G::NodeId>,
{
    type Edges = NodeFilteredEdges<'a, G, G::Edges, F>;
    fn edges(self, a: G::NodeId) -> Self::Edges {
        NodeFilteredEdges {
            graph: PhantomData,
            include_source: self.1.include_node(a),
            iter: self.0.edges(a),
            f: &self.1,
        }
    }
}


/// A filtered edges iterator.
pub struct NodeFilteredEdges<'a, G, I, F: 'a>
{
    graph: PhantomData<G>,
    include_source: bool,
    iter: I,
    f: &'a F,
}


impl<'a, G, I, F> Iterator for NodeFilteredEdges<'a, G, I, F>
    where F: FilterNode<G::NodeId>,
          G: IntoEdges,
          I: Iterator<Item=G::EdgeRef>,
{
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        if !self.include_source {
            None
        } else {
            let f = self.f;
            self.iter.find(move |&edge| f.include_node(edge.target()))
        }
    }
}

impl<G, F> DataMap for NodeFiltered<G, F>
    where G: DataMap,
          F: FilterNode<G::NodeId>,
{
    fn node_weight(&self, id: Self::NodeId) -> Option<&Self::NodeWeight> {
        if self.1.include_node(id) {
            self.0.node_weight(id)
        } else {
            None
        }
    }

    fn edge_weight(&self, id: Self::EdgeId) -> Option<&Self::EdgeWeight> {
        self.0.edge_weight(id)
    }
}

macro_rules! access0 {
    ($e:expr) => ($e.0)
}

Data!{delegate_impl [[G, F], G, NodeFiltered<G, F>, access0]}
NodeIndexable!{delegate_impl [[G, F], G, NodeFiltered<G, F>, access0]}
GraphProp!{delegate_impl [[G, F], G, NodeFiltered<G, F>, access0]}
Visitable!{delegate_impl [[G, F], G, NodeFiltered<G, F>, access0]}

/// A graph filter for edges
pub trait FilterEdge<Edge> {
    /// Return true to have the edge be part of the graph
    fn include_edge(&self, edge: Edge) -> bool;
}

impl<F, N> FilterEdge<N> for F
    where F: Fn(N) -> bool,
{
    fn include_edge(&self, n: N) -> bool {
        (*self)(n)
    }
}

/// An edge-filtering graph adaptor.
///
/// The adaptor may filter out edges. The filter implements the trait
/// `FilterEdge`. Closures of type `Fn(G::EdgeRef) -> bool` already
/// implement this trait.
///
/// The filter may use edge source, target, id, and weight to select whether to
/// include the edge or not.
#[derive(Copy, Clone, Debug)]
pub struct EdgeFiltered<G, F>(pub G, pub F);

impl<F, G> EdgeFiltered<G, F>
    where G: IntoEdgeReferences,
          F: Fn(G::EdgeRef) -> bool,
{
    /// Create an `EdgeFiltered` adaptor from the closure `filter`.
    pub fn from_fn(graph: G, filter: F) -> Self {
        EdgeFiltered(graph, filter)
    }
}

impl<G, F> GraphBase for EdgeFiltered<G, F> where G: GraphBase {
    type NodeId = G::NodeId;
    type EdgeId = G::EdgeId;
}

impl<'a, G, F> IntoNeighbors for &'a EdgeFiltered<G, F>
    where G: IntoEdges,
          F: FilterEdge<G::EdgeRef>,
{
    type Neighbors = EdgeFilteredNeighbors<'a, G, F>;
    fn neighbors(self, n: G::NodeId) -> Self::Neighbors {
        EdgeFilteredNeighbors {
            iter: self.0.edges(n),
            f: &self.1,
        }
    }
}

/// A filtered neighbors iterator.
pub struct EdgeFilteredNeighbors<'a, G, F: 'a>
    where G: IntoEdges,
{
    iter: G::Edges,
    f: &'a F,
}

impl<'a, G, F> Iterator for EdgeFilteredNeighbors<'a, G, F>
    where F: FilterEdge<G::EdgeRef>,
          G: IntoEdges,
{
    type Item = G::NodeId;
    fn next(&mut self) -> Option<Self::Item> {
        let f = self.f;
        (&mut self.iter).filter_map(move |edge| {
            if f.include_edge(edge) {
                Some(edge.target())
            } else { None }
        }).next()
    }
}

impl<'a, G, F> IntoEdgeReferences for &'a EdgeFiltered<G, F>
    where G: IntoEdgeReferences,
          F: FilterEdge<G::EdgeRef>,
{
    type EdgeRef = G::EdgeRef;
    type EdgeReferences = EdgeFilteredEdges<'a, G, G::EdgeReferences, F>;
    fn edge_references(self) -> Self::EdgeReferences {
        EdgeFilteredEdges {
            graph: PhantomData,
            iter: self.0.edge_references(),
            f: &self.1,
        }
    }
}

impl<'a, G, F> IntoEdges for &'a EdgeFiltered<G, F>
    where G: IntoEdges,
          F: FilterEdge<G::EdgeRef>,
{
    type Edges = EdgeFilteredEdges<'a, G, G::Edges, F>;
    fn edges(self, n: G::NodeId) -> Self::Edges {
        EdgeFilteredEdges {
            graph: PhantomData,
            iter: self.0.edges(n),
            f: &self.1,
        }
    }
}

/// A filtered edges iterator.
pub struct EdgeFilteredEdges<'a, G, I, F: 'a>
{
    graph: PhantomData<G>,
    iter: I,
    f: &'a F,
}

impl<'a, G, I, F> Iterator for EdgeFilteredEdges<'a, G, I, F>
    where F: FilterEdge<G::EdgeRef>,
          G: IntoEdgeReferences,
          I: Iterator<Item=G::EdgeRef>,
{
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        let f = self.f;
        self.iter.find(move |&edge| f.include_edge(edge))
    }
}

Data!{delegate_impl [[G, F], G, EdgeFiltered<G, F>, access0]}
GraphProp!{delegate_impl [[G, F], G, EdgeFiltered<G, F>, access0]}
IntoNodeIdentifiers!{delegate_impl [['a, G, F], G, &'a EdgeFiltered<G, F>, access0]}
IntoNodeReferences!{delegate_impl [['a, G, F], G, &'a EdgeFiltered<G, F>, access0]}
NodeCompactIndexable!{delegate_impl [[G, F], G, EdgeFiltered<G, F>, access0]}
NodeCount!{delegate_impl [[G, F], G, EdgeFiltered<G, F>, access0]}
NodeIndexable!{delegate_impl [[G, F], G, EdgeFiltered<G, F>, access0]}
Visitable!{delegate_impl [[G, F], G, EdgeFiltered<G, F>, access0]}
