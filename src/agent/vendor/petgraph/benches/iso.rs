#![feature(test)]

extern crate test;
extern crate petgraph;

use petgraph::prelude::*;
use petgraph::{
    EdgeType,
};
use petgraph::graph::{
    node_index,
};

/// Petersen A and B are isomorphic
///
/// http://www.dharwadker.org/tevet/isomorphism/
const PETERSEN_A: &'static str = "
 0 1 0 0 1 0 1 0 0 0 
 1 0 1 0 0 0 0 1 0 0 
 0 1 0 1 0 0 0 0 1 0 
 0 0 1 0 1 0 0 0 0 1 
 1 0 0 1 0 1 0 0 0 0 
 0 0 0 0 1 0 0 1 1 0 
 1 0 0 0 0 0 0 0 1 1 
 0 1 0 0 0 1 0 0 0 1 
 0 0 1 0 0 1 1 0 0 0 
 0 0 0 1 0 0 1 1 0 0
";

const PETERSEN_B: &'static str = "
 0 0 0 1 0 1 0 0 0 1 
 0 0 0 1 1 0 1 0 0 0 
 0 0 0 0 0 0 1 1 0 1 
 1 1 0 0 0 0 0 1 0 0
 0 1 0 0 0 0 0 0 1 1 
 1 0 0 0 0 0 1 0 1 0 
 0 1 1 0 0 1 0 0 0 0 
 0 0 1 1 0 0 0 0 1 0 
 0 0 0 0 1 1 0 1 0 0 
 1 0 1 0 1 0 0 0 0 0
";

/// An almost full set, isomorphic
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

const FULL_B: &'static str = "
 1 1 1 1 1 1 1 1 1 1 
 1 1 1 1 1 1 1 1 1 1 
 1 1 0 1 1 1 0 1 1 1 
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1 
 1 1 1 1 1 1 1 1 1 1 
 1 1 1 1 1 1 1 1 1 1 
 1 1 1 1 1 1 1 1 1 1 
 1 1 1 1 1 1 1 1 1 1 
 1 1 1 1 1 1 1 1 1 1
";

/// Praust A and B are not isomorphic
const PRAUST_A: &'static str = "
 0 1 1 1 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 
 1 0 1 1 0 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0 
 1 1 0 1 0 0 1 0 0 0 1 0 0 0 0 0 0 0 0 0 
 1 1 1 0 0 0 0 1 0 0 0 1 0 0 0 0 0 0 0 0 
 1 0 0 0 0 1 1 1 0 0 0 0 1 0 0 0 0 0 0 0 
 0 1 0 0 1 0 1 1 0 0 0 0 0 1 0 0 0 0 0 0 
 0 0 1 0 1 1 0 1 0 0 0 0 0 0 1 0 0 0 0 0 
 0 0 0 1 1 1 1 0 0 0 0 0 0 0 0 1 0 0 0 0 
 1 0 0 0 0 0 0 0 0 1 1 1 0 0 0 0 1 0 0 0 
 0 1 0 0 0 0 0 0 1 0 1 1 0 0 0 0 0 1 0 0 
 0 0 1 0 0 0 0 0 1 1 0 1 0 0 0 0 0 0 1 0 
 0 0 0 1 0 0 0 0 1 1 1 0 0 0 0 0 0 0 0 1 
 0 0 0 0 1 0 0 0 0 0 0 0 0 1 1 1 0 1 0 0 
 0 0 0 0 0 1 0 0 0 0 0 0 1 0 1 1 1 0 0 0 
 0 0 0 0 0 0 1 0 0 0 0 0 1 1 0 1 0 0 0 1 
 0 0 0 0 0 0 0 1 0 0 0 0 1 1 1 0 0 0 1 0 
 0 0 0 0 0 0 0 0 1 0 0 0 0 1 0 0 0 1 1 1 
 0 0 0 0 0 0 0 0 0 1 0 0 1 0 0 0 1 0 1 1 
 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 1 1 1 0 1 
 0 0 0 0 0 0 0 0 0 0 0 1 0 0 1 0 1 1 1 0
";

const PRAUST_B: &'static str = "
 0 1 1 1 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 
 1 0 1 1 0 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0 
 1 1 0 1 0 0 1 0 0 0 1 0 0 0 0 0 0 0 0 0 
 1 1 1 0 0 0 0 1 0 0 0 1 0 0 0 0 0 0 0 0 
 1 0 0 0 0 1 1 1 0 0 0 0 1 0 0 0 0 0 0 0 
 0 1 0 0 1 0 1 1 0 0 0 0 0 0 0 0 0 0 0 1 
 0 0 1 0 1 1 0 1 0 0 0 0 0 0 1 0 0 0 0 0 
 0 0 0 1 1 1 1 0 0 0 0 0 0 0 0 0 0 1 0 0 
 1 0 0 0 0 0 0 0 0 1 1 1 0 0 0 0 1 0 0 0
 0 1 0 0 0 0 0 0 1 0 1 1 0 1 0 0 0 0 0 0 
 0 0 1 0 0 0 0 0 1 1 0 1 0 0 0 0 0 0 1 0 
 0 0 0 1 0 0 0 0 1 1 1 0 0 0 0 1 0 0 0 0 
 0 0 0 0 1 0 0 0 0 0 0 0 0 1 1 0 0 1 0 1 
 0 0 0 0 0 0 0 0 0 1 0 0 1 0 0 1 1 0 1 0 
 0 0 0 0 0 0 1 0 0 0 0 0 1 0 0 1 0 1 0 1 
 0 0 0 0 0 0 0 0 0 0 0 1 0 1 1 0 1 0 1 0 
 0 0 0 0 0 0 0 0 1 0 0 0 0 1 0 1 0 1 1 0 
 0 0 0 0 0 0 0 1 0 0 0 0 1 0 1 0 1 0 0 1 
 0 0 0 0 0 0 0 0 0 0 1 0 0 1 0 1 1 0 0 1 
 0 0 0 0 0 1 0 0 0 0 0 0 1 0 1 0 0 1 1 0 
";

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

fn str_to_graph(s: &str) -> Graph<(), (), Undirected> {
    parse_graph(s)
}

fn str_to_digraph(s: &str) -> Graph<(), (), Directed> {
    parse_graph(s)
}

/*
fn graph_to_ad_matrix<N, E, Ty: EdgeType>(g: &Graph<N,E,Ty>)
{
    let n = g.node_count();
    for i in (0..n) {
        for j in (0..n) {
            let ix = NodeIndex::new(i);
            let jx = NodeIndex::new(j);
            let out = match g.find_edge(ix, jx) {
                None => "0",
                Some(_) => "1",
            };
            print!("{} ", out);
        }
        println!("");
    }
}
*/

#[bench]
fn petersen_iso_bench(bench: &mut test::Bencher)
{
    let a = str_to_digraph(PETERSEN_A);
    let b = str_to_digraph(PETERSEN_B);

    bench.iter(|| petgraph::algo::is_isomorphic(&a, &b));
}

#[bench]
fn petersen_undir_iso_bench(bench: &mut test::Bencher)
{
    let a = str_to_graph(PETERSEN_A);
    let b = str_to_graph(PETERSEN_B);

    bench.iter(|| petgraph::algo::is_isomorphic(&a, &b));
}

#[bench]
fn full_iso_bench(bench: &mut test::Bencher)
{
    let a = str_to_graph(FULL_A);
    let b = str_to_graph(FULL_B);

    bench.iter(|| petgraph::algo::is_isomorphic(&a, &b));
}

#[bench]
fn praust_dir_no_iso_bench(bench: &mut test::Bencher)
{
    let a = str_to_digraph(PRAUST_A);
    let b = str_to_digraph(PRAUST_B);

    bench.iter(|| petgraph::algo::is_isomorphic(&a, &b));
}

#[bench]
fn praust_undir_no_iso_bench(bench: &mut test::Bencher)
{
    let a = str_to_graph(PRAUST_A);
    let b = str_to_graph(PRAUST_B);

    bench.iter(|| petgraph::algo::is_isomorphic(&a, &b));
}

#[bench]
fn bench_praust_mst(bb: &mut test::Bencher)
{
    let a = str_to_digraph(PRAUST_A);
    let b = str_to_digraph(PRAUST_B);

    bb.iter(|| {
        (petgraph::algo::min_spanning_tree(&a),
        petgraph::algo::min_spanning_tree(&b))
    });
}
