
//! **petgraph** is a graph data structure library.
//!
//! - [`Graph`](./graph/struct.Graph.html) which is an adjacency list graph with
//! arbitrary associated data.
//!
//! - [`StableGraph`](./stable_graph/struct.StableGraph.html) is similar
//! to `Graph`, but it keeps indices stable across removals.
//!
//! - [`GraphMap`](./graphmap/struct.GraphMap.html) is an adjacency list graph
//! which is backed by a hash table and the node identifiers are the keys
//! into the table.
//! - [`CSR`](./csr/struct.Csr.html) is a sparse adjacency matrix graph with
//! arbitrary associated data.
//!
//! Optional crate feature: `"serde-1"`, see the Readme for more information.
//!
#![doc(html_root_url = "https://docs.rs/petgraph/0.4/")]

extern crate fixedbitset;
#[cfg(feature = "graphmap")]
extern crate ordermap;

#[cfg(feature = "serde-1")]
extern crate serde;
#[cfg(feature = "serde-1")]
#[macro_use]
extern crate serde_derive;

#[cfg(all(feature = "serde-1", test))]
extern crate itertools;

#[doc(no_inline)]
pub use graph::Graph;

pub use Direction::{Outgoing, Incoming};

#[macro_use]
mod macros;
mod scored;

// these modules define trait-implementing macros
#[macro_use]
pub mod visit;
#[macro_use]
pub mod data;

pub mod algo;
#[cfg(feature = "generate")]
pub mod generate;
#[cfg(feature = "graphmap")]
pub mod graphmap;
mod graph_impl;
pub mod dot;
pub mod unionfind;
mod dijkstra;
mod astar;
pub mod csr;
mod iter_format;
mod iter_utils;
mod isomorphism;
mod traits_graph;
mod util;
#[cfg(feature = "quickcheck")]
mod quickcheck;
#[cfg(feature = "serde-1")]
mod serde_utils;

pub mod prelude;

/// `Graph<N, E, Ty, Ix>` is a graph datastructure using an adjacency list representation.
pub mod graph {
    pub use graph_impl::{
        Edge,
        EdgeIndex,
        EdgeIndices,
        EdgeReference,
        EdgeReferences,
        EdgeWeightsMut,
        Edges,
        Externals,
        Frozen,
        Graph,
        Neighbors,
        Node,
        NodeIndex,
        NodeIndices,
        NodeWeightsMut,
        NodeReferences,
        WalkNeighbors,
        GraphIndex,
        IndexType,
        edge_index,
        node_index,
        DefaultIx,
        DiGraph,
        UnGraph,
    };
}

#[cfg(feature = "stable_graph")]
pub use graph_impl::stable_graph;

macro_rules! copyclone {
    ($name:ident) => {
        impl Clone for $name {
            #[inline]
            fn clone(&self) -> Self { *self }
        }
    }
}

// Index into the NodeIndex and EdgeIndex arrays
/// Edge direction.
#[derive(Copy, Debug, PartialEq, PartialOrd, Ord, Eq, Hash)]
#[repr(usize)]
pub enum Direction {
    /// An `Outgoing` edge is an outward edge *from* the current node.
    Outgoing = 0,
    /// An `Incoming` edge is an inbound edge *to* the current node.
    Incoming = 1
}

copyclone!(Direction);

impl Direction {
    /// Return the opposite `Direction`.
    #[inline]
    pub fn opposite(&self) -> Direction {
        match *self {
            Outgoing => Incoming,
            Incoming => Outgoing,
        }
    }

    /// Return `0` for `Outgoing` and `1` for `Incoming`.
    #[inline]
    pub fn index(&self) -> usize {
        (*self as usize) & 0x1
    }
}

#[doc(hidden)]
pub use Direction as EdgeDirection;

/// Marker type for a directed graph.
#[derive(Copy, Debug)]
pub enum Directed { }
copyclone!(Directed);

/// Marker type for an undirected graph.
#[derive(Copy, Debug)]
pub enum Undirected { }
copyclone!(Undirected);

/// A graph's edge type determines whether is has directed edges or not.
pub trait EdgeType {
    fn is_directed() -> bool;
}

impl EdgeType for Directed {
    #[inline]
    fn is_directed() -> bool { true }
}

impl EdgeType for Undirected {
    #[inline]
    fn is_directed() -> bool { false }
}


/// Convert an element like `(i, j)` or `(i, j, w)` into
/// a triple of source, target, edge weight.
///
/// For `Graph::from_edges` and `GraphMap::from_edges`.
pub trait IntoWeightedEdge<E> {
    type NodeId;
    fn into_weighted_edge(self) -> (Self::NodeId, Self::NodeId, E);
}

impl<Ix, E> IntoWeightedEdge<E> for (Ix, Ix)
    where E: Default
{
    type NodeId = Ix;

    fn into_weighted_edge(self) -> (Ix, Ix, E) {
        let (s, t) = self;
        (s, t, E::default())
    }
}

impl<Ix, E> IntoWeightedEdge<E> for (Ix, Ix, E)
{
    type NodeId = Ix;
    fn into_weighted_edge(self) -> (Ix, Ix, E) {
        self
    }
}

impl<'a, Ix, E> IntoWeightedEdge<E> for (Ix, Ix, &'a E)
    where E: Clone
{
    type NodeId = Ix;
    fn into_weighted_edge(self) -> (Ix, Ix, E) {
        let (a, b, c) = self;
        (a, b, c.clone())
    }
}

impl<'a, Ix, E> IntoWeightedEdge<E> for &'a (Ix, Ix)
    where Ix: Copy, E: Default
{
    type NodeId = Ix;
    fn into_weighted_edge(self) -> (Ix, Ix, E) {
        let (s, t) = *self;
        (s, t, E::default())
    }
}

impl<'a, Ix, E> IntoWeightedEdge<E> for &'a (Ix, Ix, E)
    where Ix: Copy, E: Clone
{
    type NodeId = Ix;
    fn into_weighted_edge(self) -> (Ix, Ix, E) {
        self.clone()
    }
}
