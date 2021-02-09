//! Simple graphviz dot file format output.

use std::fmt::{self, Display, Write};

use visit::{GraphRef};

/// `Dot` implements output to graphviz .dot format for a graph.
///
/// Formatting and options are rather simple, this is mostly intended
/// for debugging. Exact output may change.
///
/// # Examples
///
/// ```
/// use petgraph::Graph;
/// use petgraph::dot::{Dot, Config};
///
/// let mut graph = Graph::<_, ()>::new();
/// graph.add_node("A");
/// graph.add_node("B");
/// graph.add_node("C");
/// graph.add_node("D");
/// graph.extend_with_edges(&[
///     (0, 1), (0, 2), (0, 3),
///     (1, 2), (1, 3),
///     (2, 3),
/// ]);
///
/// println!("{:?}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));
///
/// // In this case the output looks like this:
/// // 
/// // digraph {
/// //     0 [label="\"A\""]
/// //     1 [label="\"B\""]
/// //     2 [label="\"C\""]
/// //     3 [label="\"D\""]
/// //     0 -> 1
/// //     0 -> 2
/// //     0 -> 3
/// //     1 -> 2
/// //     1 -> 3
/// //     2 -> 3
/// // }
///
/// // If you need multiple config options, just list them all in the slice.
/// ```
pub struct Dot<'a, G> {
    graph: G,
    config: &'a [Config],
}

static TYPE: [&'static str; 2] = ["graph", "digraph"];
static EDGE: [&'static str; 2] = ["--", "->"];
static INDENT: &'static str = "    ";

impl<'a, G> Dot<'a, G> where G: GraphRef {
    /// Create a `Dot` formatting wrapper with default configuration.
    pub fn new(graph: G) -> Self {
        Self::with_config(graph, &[])
    }

    /// Create a `Dot` formatting wrapper with custom configuration.
    pub fn with_config(graph: G, config: &'a [Config]) -> Self {
        Dot {
            graph: graph,
            config: config,
        }
    }
}

/// `Dot` configuration.
///
/// This enum does not have an exhaustive definition (will be expanded)
#[derive(Debug, PartialEq, Eq)]
pub enum Config {
    /// Use indices for node labels.
    NodeIndexLabel,
    /// Use indices for edge labels.
    EdgeIndexLabel,
    /// Use no edge labels.
    EdgeNoLabel,
    #[doc(hidden)]
    _Incomplete(()),
}

use visit::{ IntoNodeReferences, NodeIndexable, IntoEdgeReferences, EdgeRef};
use visit::{ Data, NodeRef, GraphProp, };

impl<'a, G> Dot<'a, G>
{
    fn graph_fmt<NF, EF, NW, EW>(&self, g: G, f: &mut fmt::Formatter,
                    mut node_fmt: NF, mut edge_fmt: EF) -> fmt::Result
        where G: NodeIndexable + IntoNodeReferences + IntoEdgeReferences,
              G: GraphProp,
              G: Data<NodeWeight=NW, EdgeWeight=EW>,
              NF: FnMut(&NW, &mut FnMut(&Display) -> fmt::Result) -> fmt::Result,
              EF: FnMut(&EW, &mut FnMut(&Display) -> fmt::Result) -> fmt::Result,
    {
        try!(writeln!(f, "{} {{", TYPE[g.is_directed() as usize]));

        // output all labels
        for node in g.node_references() {
            try!(write!(f, "{}{}", INDENT, g.to_index(node.id())));
            if self.config.contains(&Config::NodeIndexLabel) {
                try!(writeln!(f, ""));
            } else {
                try!(write!(f, " [label=\""));
                try!(node_fmt(node.weight(), &mut |d| Escaped(d).fmt(f)));
                try!(writeln!(f, "\"]"));
            }

        }
        // output all edges
        for (i, edge) in g.edge_references().enumerate() {
            try!(write!(f, "{}{} {} {}",
                        INDENT,
                        g.to_index(edge.source()),
                        EDGE[g.is_directed() as usize],
                        g.to_index(edge.target())));
            if self.config.contains(&Config::EdgeNoLabel) {
                try!(writeln!(f, ""));
            } else if self.config.contains(&Config::EdgeIndexLabel) {
                try!(writeln!(f, " [label=\"{}\"]", i));
            } else {
                try!(write!(f, " [label=\""));
                try!(edge_fmt(edge.weight(), &mut |d| Escaped(d).fmt(f)));
                try!(writeln!(f, "\"]"));
            }
        }

        try!(writeln!(f, "}}"));
        Ok(())
    }
}

impl<'a, G> fmt::Display for Dot<'a, G>
    where G: IntoEdgeReferences + IntoNodeReferences + NodeIndexable + GraphProp,
          G::EdgeWeight: fmt::Display,
          G::NodeWeight: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.graph_fmt(self.graph, f, |n, cb| cb(n), |e, cb| cb(e))
    }
}

impl<'a, G> fmt::Debug for Dot<'a, G>
    where G: IntoEdgeReferences + IntoNodeReferences + NodeIndexable + GraphProp,
          G::EdgeWeight: fmt::Debug,
          G::NodeWeight: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.graph_fmt(self.graph, f,
                       |n, cb| cb(&DebugFmt(n)),
                       |e, cb| cb(&DebugFmt(e)))
    }
}

/// Escape for Graphviz
struct Escaper<W>(W);

impl<W> fmt::Write for Escaper<W>
    where W: fmt::Write
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            try!(self.write_char(c));
        }
        Ok(())
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        match c {
            '"' => try!(self.0.write_char('\\')),
            // \l is for left justified linebreak
            '\n' => return self.0.write_str(r#"\l"#),
            _   => { }
        }
        self.0.write_char(c)
    }
}

/// Pass Display formatting through a simple escaping filter
struct Escaped<T>(T);

impl<T> fmt::Display for Escaped<T>
    where T: fmt::Display
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if f.alternate() {
            write!(&mut Escaper(f), "{:#}\\l", &self.0)
        } else {
            write!(&mut Escaper(f), "{}", &self.0)
        }
    }
}

/// Pass Debug formatting to Display
struct DebugFmt<T>(T);

impl<T> fmt::Display for DebugFmt<T>
    where T: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}
