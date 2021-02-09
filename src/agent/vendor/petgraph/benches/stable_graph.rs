#![feature(test)]

extern crate petgraph;
extern crate test;

use test::Bencher;
use petgraph::prelude::*;

use petgraph::{EdgeType};
use petgraph::stable_graph::{
    node_index,
};

/// An almost full set
const FULL_A: &'static str = "
 1 1 1 1 1 1 1 1 1 1 
 1 1 1 1 1 1 1 1 1 1 
 1 1 1 1 1 1 1 1 1 1 
 1 1 1 1 1 1 1 1 1 1 
 1 1 1 1 1 1 1 1 1 1 
 1 1 1 1 1 1 1 1 1 1 
 1 1 1 1 1 1 1 1 1 1 
 1 1 1 1 1 1 1 1 1 1 
 1 1 1 1 0 1 1 1 0 1 
 1 1 1 1 1 1 1 1 1 1
";

const BIGGER: &'static str = "
 0 0 0 0 0 0 0 0 1 0 1 0 0 0 0 0 0 0 1 1 0 1 1 1 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 1 0 0 0 1 1 0 0 0 0 0 0 0 1 0 1 0 1 1 0 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0
 0 1 0 1 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 1 1 0 1 0 0 1 0 0 0 1 0 0 0 0 0 0 0 0 0
 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 1 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 1 0 1 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 1 1 1 0 1 0 0
 0 0 0 0 0 1 0 0 0 0 0 0 1 0 0 1 1 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 1 0 1 1 1 0 0 0
 0 0 0 0 0 0 1 0 0 0 0 0 1 1 0 1 0 0 0 1 0 0 0 0 0 0 1 0 0 0 0 0 1 1 0 1 0 0 0 1
 0 0 0 0 0 0 0 1 0 0 0 0 1 1 1 0 0 0 1 0 0 0 0 0 0 0 0 1 0 0 0 0 1 1 1 0 0 0 1 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 1 1 1 0 1 1 0 0 0 0 0 1 0 0 0 0 1 0 0 0 1 1 1
 0 0 0 0 1 0 0 0 0 0 0 0 1 0 0 0 1 0 1 1 0 1 1 1 0 0 0 0 0 1 0 0 1 0 0 0 1 0 1 1
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 1 1 1 0 1
 0 0 0 0 0 0 0 0 0 0 0 1 0 0 1 0 1 1 1 1 0 0 0 0 0 0 0 0 0 0 0 1 0 0 1 0 1 1 1 0
 0 1 1 0 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 1 1 1 1 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0
 1 0 1 0 0 1 0 0 0 1 0 0 0 0 0 0 0 1 1 0 1 0 1 1 0 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 1 0 1 1 0 1 0 0 0 0 0 0 1 0 0 0 0 0 0 0 1 0 1 1 0 1 0 0 0 0 0 0 1 0 0 0 0 0
 0 0 0 0 1 1 1 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 1 1 1 1 0 0 0 0 0 0 0 0 1 0 0 0 0
 1 0 0 0 0 0 0 0 0 1 1 1 0 0 0 0 1 0 0 0 1 0 0 0 0 0 0 0 0 1 1 1 0 0 0 0 1 0 0 0
 0 1 0 0 0 0 0 0 1 0 1 1 0 0 0 0 0 1 0 0 0 1 0 0 0 0 0 0 1 0 1 1 0 0 0 0 0 1 0 0
 0 0 1 0 0 0 0 0 1 1 0 1 0 0 0 0 0 0 1 0 0 0 1 0 0 0 0 0 1 1 0 1 0 0 0 0 0 0 1 0
 0 0 0 1 0 0 0 0 1 1 1 0 0 0 0 0 0 0 0 1 0 0 0 1 0 0 0 0 1 1 1 0 0 0 0 0 0 0 0 1
 0 0 0 0 1 0 0 0 0 0 0 0 0 1 1 1 0 1 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 1 1 1 0 1 0 0
 0 0 0 0 0 1 0 0 0 0 0 0 1 0 1 1 1 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 1 0 1 1 1 0 0 0
 0 1 0 0 0 0 1 0 0 0 0 0 1 1 0 1 0 0 0 1 0 0 0 0 0 0 1 0 0 0 0 0 1 1 0 1 0 0 0 1
 0 1 0 0 0 0 0 1 0 0 0 0 1 1 1 0 0 0 1 0 0 0 0 0 0 0 0 1 0 0 0 0 1 1 1 0 0 0 1 0
 0 1 0 0 0 0 0 0 1 0 0 0 0 1 0 0 0 1 1 1 0 0 0 0 0 0 0 0 1 0 0 0 0 1 0 0 0 1 1 1
 0 1 0 0 0 0 0 0 0 1 0 0 1 0 0 0 1 0 1 1 0 0 0 0 0 0 0 0 0 1 0 0 1 0 0 0 1 0 1 1
 0 1 0 0 0 0 0 0 0 0 1 0 0 0 0 1 1 1 0 1 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 1 1 1 0 1
 0 1 0 0 0 0 0 0 0 0 0 1 0 0 1 0 1 1 1 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 1 0 1 1 1 0
";

#[bench]
fn full_edges_default(bench: &mut Bencher)
{
    let a = parse_stable_graph::<Directed>(FULL_A);

    bench.iter(|| a.edges(node_index(1)).count())
}

#[bench]
fn full_edges_out(bench: &mut Bencher)
{
    let a = parse_stable_graph::<Directed>(FULL_A);
    bench.iter(|| a.edges_directed(node_index(1), Outgoing).count())
}
#[bench]
fn full_edges_in(bench: &mut Bencher)
{
    let a = parse_stable_graph::<Directed>(FULL_A);

    bench.iter(|| a.edges_directed(node_index(1), Incoming).count())
}

#[bench]
fn neighbors_default(bench: &mut Bencher)
{
    let a = parse_stable_graph::<Directed>(FULL_A);

    bench.iter(|| a.neighbors(node_index(1)).count())
}

#[bench]
fn neighbors_out(bench: &mut Bencher)
{
    let a = parse_stable_graph::<Directed>(FULL_A);
    bench.iter(|| a.neighbors_directed(node_index(1), Outgoing).count())
}
#[bench]
fn neighbors_in(bench: &mut Bencher)
{
    let a = parse_stable_graph::<Directed>(FULL_A);

    bench.iter(|| a.neighbors_directed(node_index(1), Incoming).count())
}

#[bench]
fn sccs_stable_graph(bench: &mut Bencher)
{
    let a = parse_stable_graph::<Directed>(BIGGER);
    bench.iter(|| petgraph::algo::kosaraju_scc(&a));
}

#[bench]
fn sccs_graph(bench: &mut Bencher)
{
    let a = parse_graph::<Directed>(BIGGER);
    bench.iter(|| petgraph::algo::kosaraju_scc(&a));
}

/// Parse a text adjacency matrix format into a directed graph
fn parse_stable_graph<Ty: EdgeType>(s: &str) -> StableGraph<(), (), Ty>
{
    let mut gr = StableGraph::default();
    let s = s.trim();
    let lines = s.lines().filter(|l| !l.is_empty());
    for (row, line) in lines.enumerate() {
        for (col, word) in line.split(' ')
                                .filter(|s| s.len() > 0)
                                .enumerate()
        {
            let has_edge = word.parse::<i32>().unwrap();
            assert!(has_edge == 0 || has_edge == 1);
            if has_edge == 0 {
                continue;
            }
            while col >= gr.node_count() || row >= gr.node_count() {
                gr.add_node(());
            }
            gr.update_edge(node_index(row), node_index(col), ());
        }
    }
    gr
}

/// Parse a text adjacency matrix format into a directed graph
fn parse_graph<Ty: EdgeType>(s: &str) -> Graph<(), (), Ty>
{
    let mut gr = Graph::with_capacity(0, 0);
    let s = s.trim();
    let lines = s.lines().filter(|l| !l.is_empty());
    for (row, line) in lines.enumerate() {
        for (col, word) in line.split(' ')
                                .filter(|s| s.len() > 0)
                                .enumerate()
        {
            let has_edge = word.parse::<i32>().unwrap();
            assert!(has_edge == 0 || has_edge == 1);
            if has_edge == 0 {
                continue;
            }
            while col >= gr.node_count() || row >= gr.node_count() {
                gr.add_node(());
            }
            gr.update_edge(node_index(row), node_index(col), ());
        }
    }
    gr
}


#[bench]
fn stable_graph_map(bench: &mut Bencher)
{
    let a = parse_stable_graph::<Directed>(BIGGER);
    bench.iter(|| a.map(|i, _| i, |i, _| i));
}

#[bench]
fn graph_map(bench: &mut Bencher)
{
    let a = parse_graph::<Directed>(BIGGER);
    bench.iter(|| a.map(|i, _| i, |i, _| i));
}

#[bench]
fn stable_graph_retain_nodes(bench: &mut Bencher)
{
    let mut a = parse_stable_graph::<Directed>(BIGGER);
    bench.iter(|| a.retain_nodes(|gr, i| (i.index() + 1) % 3700 != 0));
}

#[bench]
fn stable_graph_retain_edges(bench: &mut Bencher)
{
    let mut a = parse_stable_graph::<Directed>(BIGGER);
    bench.iter(|| a.retain_edges(|gr, i| (i.index() + 1) % 3700 != 0));
}
