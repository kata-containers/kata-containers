
petgraph
========

Graph data structure library. Requires Rust 1.12.

Please read the `API documentation here`__

__ https://docs.rs/petgraph/

|build_status|_ |crates|_

.. |build_status| image:: https://travis-ci.org/bluss/petgraph.svg?branch=master
.. _build_status: https://travis-ci.org/bluss/petgraph

.. |crates| image:: http://meritbadge.herokuapp.com/petgraph
.. _crates: https://crates.io/crates/petgraph

Crate feature flags:

- ``graphmap`` (default) enable ``GraphMap``.
- ``stable_graph`` (default) enable ``StableGraph``.
- ``serde-1`` (optional) enable serialization for ``Graph, StableGraph`` using
  serde 1.0. Requires Rust version as required by serde.

Recent Changes
--------------

- 0.4.13

  - Fix clippy warnings by @jonasbb
  - Add docs for ``Csr`` by @ksadorf
  - Fix conflict with new stable method ``find_map`` in new Rust

- 0.4.12

  - Newtype ``Time`` now also implements ``Hash``
  - Documentation updates for ``Frozen``.

- 0.4.11

  - Fix ``petgraph::graph::NodeReferences`` to be publically visible
  - Small doc typo and code style files by @shepmaster and @waywardmonkeys
  - Fix a future compat warning with pointer casts

- 0.4.10

  - Add graph trait ``IntoEdgesDirected``
  - Update dependencies

- 0.4.9

  - Fix ``bellman_ford`` to work correctly with undirected graphs (#152) by
    @carrutstick
  - Performance improvements for ``Graph, Stablegraph``'s ``.map()``.

- 0.4.8

  - ``StableGraph`` learned new methods nearing parity with ``Graph``.  Note
    that the ``StableGraph`` methods preserve index stability even in the batch
    removal methods like ``filter_map`` and ``retain_edges``.

    + Added ``.filter_map()``, which maps associated node and edge data
    + Added ``.retain_edges()``, ``.edge_indices()`` and ``.clear_edges()``

  - Existing ``Graph`` iterators gained some trait impls:

    + ``.node_indices(), .edge_indices()`` are ``ExactSizeIterator``
    + ``.node_references()`` is now
      ``DoubleEndedIterator + ExactSizeIterator``.
    + ``.edge_references()`` is now ``ExactSizeIterator``.

  - Implemented ``From<StableGraph>`` for ``Graph``.

- 0.4.7

  - New algorithm by @jmcomets: A* search algorithm in ``petgraph::algo::astar``
  - One ``StableGraph`` bug fix whose patch was supposed to be in the previous
    version:

    + ``add_edge(m, n, _)`` now properly always panics if nodes m or n don't
      exist in the graph.

- 0.4.6

  - New optional crate feature: ``"serde-1"``, which enables serialization
    for ``Graph`` and ``StableGraph`` using serde.
  - Add methods ``new``, ``add_node`` to ``Csr`` by @jmcomets
  - Add indexing with ``[]`` by node index, ``NodeCompactIndexable`` for
    ``Csr`` by @jmcomets
  - Amend doc for ``GraphMap::into_graph`` (it has a case where it can panic)
  - Add implementation of ``From<Graph>`` for ``StableGraph``.
  - Add implementation of ``IntoNodeReferences`` for ``&StableGraph``.
  - Add method ``StableGraph::map`` that maps associated data
  - Add method ``StableGraph::find_edge_undirected``
  - Many ``StableGraph`` bug fixes involving node vacancies (holes left by
    deletions):

    + ``neighbors(n)`` and similar neighbor and edge iterator methods now
      handle n being a vacancy properly. (This produces an empty iterator.)
    + ``find_edge(m, n)`` now handles m being a vacancy correctly too
    + ``StableGraph::node_bound`` was fixed for empty graphs and returns 0

  - Add implementation of ``DoubleEndedIterator`` to ``Graph, StableGraph``'s
    edge references iterators.
  - Debug output for ``Graph`` now shows node and edge count. ``Graph, StableGraph``
    show nothing for the edges list if it's empty (no label).
  - ``Arbitrary`` implementation for ``StableGraph`` now can produce graphs with
    vacancies (used by quickcheck)

- 0.4.5

  - Fix ``max`` ambiguity error with current rust nightly by @daboross (#153)

- 0.4.4

  - Add ``GraphMap::all_edges_mut()`` iterator by @Binero
  - Add ``StableGraph::retain_nodes`` by @Rupsbant
  - Add ``StableGraph::index_twice_mut`` by @christolliday

- 0.4.3

  - Add crate categories

- 0.4.2

  - Move the ``visit.rs`` file due to changed rules for a module’s directory
    ownership in Rust, resolving a future compat warning.
  - The error types ``Cycle, NegativeCycle`` now implement ``PartialEq``.

- 0.4.1

  - Add new algorithm ``simple_fast`` for computing dominators in a control-flow
    graph.

- 0.4.0

  - Breaking changes in ``Graph``:

    - ``Graph::edges`` and the other edges methods now return an iterator of
      edge references

  - Other breaking changes:

    - ``toposort`` now returns an error if the graph had a cycle.
    - ``is_cyclic_directed`` no longer takes a dfs space argument. It is
      now recursive.
    - ``scc`` was renamed to ``kosaraju_scc``.
    - ``min_spanning_tree`` now returns an iterator that needs to be
      made into a specific graph type deliberately.
    - ``dijkstra`` now uses the ``IntoEdges`` trait.
    - ``NodeIndexable`` changed its method signatures.
    - ``IntoExternals`` was removed, and many other smaller adjustments
      in graph traits. ``NodeId`` must now implement ``PartialEq``, for example.
    - ``DfsIter, BfsIter`` were removed in favour of a more general approach
      with the ``Walker`` trait and its iterator conversion.

  - New features:

    - New graph traits, for example ``IntoEdges`` which returns
      an iterator of edge references. Everything implements the graph traits
      much more consistently.
    - Traits for associated data access and building graphs: ``DataMap``,
      ``Build, Create, FromElements``.
    - Graph adaptors: ``EdgeFiltered``. ``Filtered`` was renamed to ``NodeFiltered``.
    - New algorithms: bellman-ford
    - New graph: compressed sparse row (``Csr``).
    - ``GraphMap`` implements ``NodeIndexable``.
    - ``Dot`` was generalized

- 0.3.2

  - Add ``depth_first_search``, a recursive dfs visitor that emits discovery,
    finishing and edge classification events.
  - Add graph adaptor ``Filtered``.
  - impl ``Debug, NodeIndexable`` for ``Reversed``.

- 0.3.1

  - Add ``.edges(), .edges_directed()`` to ``StableGraph``. Note that these
    differ from ``Graph``, because this is the signature they will all use
    in the future.
  - Add ``.update_edge()`` to ``StableGraph``.
  - Add reexports of common items in ``stable_graph`` module (for example
    ``NodeIndex``).
  - Minor performance improvements to graph iteration
  - Improved docs for ``visit`` module.

- 0.3.0

  - Overhaul all graph visitor traits so that they use the ``IntoIterator``
    style. This makes them composable.

    - Multiple graph algorithms use new visitor traits.
    - **Help is welcome to port more algorithms (and create new graph traits in
      the process)!**

  - ``GraphMap`` can now have directed edges. ``GraphMap::new`` is now generic
    in the edge type. ``DiGraphMap`` and ``UnGraphMap`` are new type aliases.
  - Add type aliases ``DiGraph, UnGraph, StableDiGraph, StableUnGraph``
  - ``GraphMap`` is based on the ordermap crate. Deterministic iteration
    order, faster iteration, no side tables needed to convert to ``Graph``.
  - Improved docs for a lot of types and functions.
  - Add graph visitor ``DfsPostOrder``
  - ``Dfs`` gained new methods ``from_parts`` and ``reset``.
  - New algo ``has_path_connecting``.
  - New algo ``tarjan_scc``, a second scc implementation.
  - Document traversal order in ``Dfs, DfsPostOrder, scc, tarjan_scc``.
  - Optional graph visitor workspace reuse in ``has_path_connecting``,
    ``is_cyclic_directed, toposort``.
  - Improved ``Debug`` formatting for ``Graph, StableGraph``.
  - Add a prelude module
  - ``GraphMap`` now has a method ``.into_graph()`` that makes a ``Graph``.
  - ``Graph::retain_nodes, retain_edges`` now expose the self graph only
    as wrapped in ``Frozen``, so that weights can be mutated but the
    graph structure not.
  - Enable ``StableGraph`` by default
  - Add method ``Graph::contains_edge``.
  - Renamed ``EdgeDirection`` → ``Direction``.
  - Remove ``SubTopo``.
  - Require Rust 1.12 or later

- 0.2.10

  - Fix compilation with rust nightly

- 0.2.9

  - Fix a bug in SubTopo (#81)

- 0.2.8

  - Add Graph methods reserve_nodes, reserve_edges, reserve_exact_nodes,
    reserve_exact_edges, shrink_to_fit_edges, shrink_to_fit_nodes, shrink_to_fit

- 0.2.7

  - Update URLs

- 0.2.6

  - Fix warning about type parameter defaults (no functional change)

- 0.2.5

  - Add SubTopo, a topo walker for the subgraph reachable from a starting point.
  - Add condensation, which forms the graph of a graph’s strongly connected
    components.

- 0.2.4

  - Fix an algorithm error in scc (#61). This time we have a test that
    crosschecks the result of the algorithm vs another implementation, for
    greater confidence in its correctness.

- 0.2.3

  - Require Rust 1.6: Due to changes in how rust uses type parameter defaults.
  - Implement Graph::clone_from.

- 0.2.2

  - Require Rust 1.5
  - ``Dot`` passes on the alternate flag to node and edge label formatting
  - Add ``Clone`` impl for some iterators
  - Document edge iteration order for ``Graph::neighbors``
  - Add *experimental feature* ``StableGraph``, using feature flag ``stable_graph``

- 0.2.1

  - Add algorithm ``is_isomorphic_matching``

- 0.2.0

  - New Features

    - Add Graph::neighbors().detach() to step edges without borrowing.
      This is more general than, and replaces now deprecated
      walk_edges_directed. (#39)
    - Implement Default for Graph, GraphMap
    - Add method EdgeDirection::opposite()

  - Breaking changes

    - Graph::neighbors() for undirected graphs and Graph::neighbors_undirected
      for any graph now visit self loop edges once, not twice. (#31)
    - Renamed Graph::without_edges to Graph::externals
    - Removed Graph::edges_both
    - GraphMap::add_edge now returns ``Option<E>``
    - Element type of ``GraphMap<N, E>::all_edges()`` changed to ``(N, N, &E)``

  - Minor breaking changes

    - IntoWeightedEdge changed a type parameter to associated type
    - IndexType is now an unsafe trait
    - Removed IndexType::{one, zero}, use method new instead.
    - Removed MinScored
    - Ptr moved to the graphmap module.
    - Directed, Undirected are now void enums.
    - Fields of graphmap::Edges are now private (#19)

- 0.1.18

  - Fix bug on calling GraphMap::add_edge with existing edge (#35)

- 0.1.17

  - Add Graph::capacity(), GraphMap::capacity()
  - Fix bug in Graph::reverse()
  - Graph and GraphMap have `quickcheck::Arbitrary` implementations,
    if optional feature `quickcheck` is enabled.

- 0.1.16

  - Add Graph::node_indices(), Graph::edge_indices()
  - Add Graph::retain_nodes(), Graph::retain_edges()
  - Add Graph::extend_with_edges(), Graph::from_edges()
  - Add functions petgraph::graph::{edge_index, node_index};
  - Add GraphMap::extend(), GraphMap::from_edges()
  - Add petgraph::dot::Dot for simple graphviz dot output

- 0.1.15

  - Add Graph::clear_edges()
  - Add Graph::edge_endpoints()
  - Add Graph::map() and Graph::filter_map()

- 0.1.14

  - Add new topological order visitor Topo
  - New graph traits NeighborsDirected, Externals, Revisitable

- 0.1.13

  - Add iterator GraphMap::all_edges

- 0.1.12

  - Fix an algorithm error in scc (#14)

- 0.1.11

  - Update for well-formedness warnings (Rust RFC 1214), adding
    new lifetime bounds on NeighborIter and Dfs, impact should be minimal.

- 0.1.10
  
  - Fix bug in WalkEdges::next_neighbor()

- 0.1.9

  - Fix Dfs/Bfs for a rustc bugfix that disallowed them
  - Add method next_neighbor() to WalkEdges

- 0.1.8

  - Add Graph::walk_edges_directed()
  - Add Graph::index_twice_mut()

- 0.1.7

  - Add Graph::edges_directed()

- 0.1.6

  - Add Graph::node_weights_mut and Graph::edge_weights_mut

- 0.1.4

  - Add back DfsIter, BfsIter

License
-------

Dual-licensed to be compatible with the Rust project.

Licensed under the Apache License, Version 2.0
http://www.apache.org/licenses/LICENSE-2.0 or the MIT license
http://opensource.org/licenses/MIT, at your
option. This file may not be copied, modified, or distributed
except according to those terms.


