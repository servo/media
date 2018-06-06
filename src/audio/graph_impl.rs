use audio::block::Block;
use petgraph::stable_graph::NodeIndex;
use std::cell::RefCell;
use audio::node::AudioNodeEngine;
use audio::destination_node::DestinationNode;
use petgraph::stable_graph::StableGraph;
use petgraph::graph::DefaultIx;
use petgraph::visit::EdgeRef;

#[derive(Clone, Copy)]
pub struct NodeId(pub usize);

// we'll later alias NodeId to this
pub type LocalNodeId = NodeIndex<DefaultIx>;

/// A zero-indexed "port" for a node. Most nodes have one
/// input and one output port, but some may have more
///
/// Kind is a zero sized type and is useful for distinguishing
/// between input and output ports (which may otherwise share indices)
pub type PortIndex<Kind> = (u32, Kind);

/// An identifier for a port.
pub type PortId<Kind> = (LocalNodeId, PortIndex<Kind>);

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct InputPort;
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct OutputPort;

pub struct GraphImpl {
    graph: StableGraph<Node, Edge>,
    dest_id: LocalNodeId,
}

pub struct Node {
    node: Box<AudioNodeEngine>,
}

/// Edges go *to* the output port from the input port
///
/// The edge direction is the *reverse* of the direction of sound
/// since we need to do a postorder DFS traversal starting at the output
pub struct Edge {
    input_idx: PortIndex<InputPort>,
    output_idx: PortIndex<OutputPort>,
    cache: RefCell<Option<Block>>,
}

impl GraphImpl {
    pub fn new() -> Self {
        let mut graph = StableGraph::new();
        let dest_id = graph.add_node(Node::new(Box::new(DestinationNode)));
        GraphImpl { graph, dest_id }
    }

    pub fn add_node(&mut self, node: Box<AudioNodeEngine>) -> LocalNodeId {
        self.graph.add_node(Node::new(node))
    }

    pub fn add_edge(&mut self, out: PortId<OutputPort>, inp: PortId<InputPort>) {
        // Output ports can only have a single edge associated with them.
        // Remove all others
        let old = self
            .graph
            .edges(out.0)
            .find(|e| e.weight().input_idx == inp.1)
            .map(|e| e.id());
        if let Some(old) = old {
            self.graph.remove_edge(old);
        }
        // add a new edge
        // XXXManishearth it is actually possible for two nodes to have multiple edges between them between
        // different ports. We should represent this somehow.
        self.graph.add_edge(inp.0, out.0, Edge::new(inp.1, out.1));
    }

    pub fn dest_id(&self) -> LocalNodeId {
        self.dest_id
    }
}

impl Node {
    pub fn new(node: Box<AudioNodeEngine>) -> Self {
        Node { node }
    }
}

impl Edge {
    pub fn new(input_idx: PortIndex<InputPort>, output_idx: PortIndex<OutputPort>) -> Self {
        Edge {
            input_idx,
            output_idx,
            cache: RefCell::new(None),
        }
    }
}
