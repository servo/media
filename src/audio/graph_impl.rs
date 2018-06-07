use audio::block::Block;
use audio::block::Chunk;
use audio::destination_node::DestinationNode;
use audio::node::AudioNodeEngine;
use audio::node::BlockInfo;
use petgraph::Direction;
use petgraph::graph::DefaultIx;
use petgraph::stable_graph::NodeIndex;
use petgraph::stable_graph::StableGraph;
use petgraph::visit::{DfsPostOrder, EdgeRef};
use std::cell::RefCell;

#[derive(Clone, Copy)]
pub struct NodeId(pub usize);

// we'll later alias NodeId to this
pub type LocalNodeId = NodeIndex<DefaultIx>;

/// A zero-indexed "port" for a node. Most nodes have one
/// input and one output port, but some may have more.
/// For example, a channel splitter node will have one output
/// port for each channel.
///
/// These are essentially indices into the Chunks
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
    node: RefCell<Box<AudioNodeEngine>>,
}

/// Edges go *to* the output port from the input port
///
/// The edge direction is the *reverse* of the direction of sound
/// since we need to do a postorder DFS traversal starting at the output
pub struct Edge {
    /// The index of the port on the input node
    /// This is actually the /output/ of this edge
    input_idx: PortIndex<InputPort>,
    /// The index of the port on the output node
    /// This is actually the /input/ of this edge
    output_idx: PortIndex<OutputPort>,
    cache: RefCell<Option<Block>>,
}

impl GraphImpl {
    pub fn new() -> Self {
        let mut graph = StableGraph::new();
        let dest_id = graph.add_node(Node::new(Box::new(DestinationNode::new())));
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
        // XXXManishearth it is actually possible for two nodes to have
        // multiple edges between them between
        // different ports. We should represent this somehow.
        self.graph.add_edge(inp.0, out.0, Edge::new(inp.1, out.1));
    }

    pub fn dest_id(&self) -> LocalNodeId {
        self.dest_id
    }

    pub fn process(&mut self, info: &BlockInfo) -> Chunk {
        let mut visit = DfsPostOrder::new(&self.graph, self.dest_id);
        while let Some(ix) = visit.next(&self.graph) {
            let mut curr = self.graph[ix].node.borrow_mut();
            let mut chunk = Chunk::default();
            if curr.input_count() > 0 {
                chunk
                    .blocks
                    .resize(curr.input_count() as usize, Default::default());
                for edge in self.graph.edges(ix) {
                    let edge = edge.weight();
                    // XXXManishearth we can have multiple edges
                    // hitting the same input port, we should deal with that
                    chunk[edge.input_idx] = edge
                        .cache
                        .borrow_mut()
                        .take()
                        .expect("Cache should have been filled from traversal");
                }
            }
            let mut out = curr.process(chunk, info);

            assert!(out.len() == curr.output_count() as usize);
            if curr.output_count() == 0 {
                continue;
            }

            for edge in self.graph.edges_directed(ix, Direction::Incoming) {
                let edge = edge.weight();
                *edge.cache.borrow_mut() = Some(out[edge.output_idx].take());
            }
        }
        self.graph[self.dest_id].node.borrow_mut()
            .destination_data().expect("Destination node should have data cached")
    }
}

impl Node {
    pub fn new(node: Box<AudioNodeEngine>) -> Self {
        Node {
            node: RefCell::new(node),
        }
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
