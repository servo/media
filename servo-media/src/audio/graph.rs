use audio::block::{Block, Chunk};
use audio::destination_node::DestinationNode;
use audio::node::{AudioNodeEngine, BlockInfo, ChannelCountMode};
use petgraph::Direction;
use petgraph::graph::DefaultIx;
use petgraph::stable_graph::NodeIndex;
use petgraph::stable_graph::StableGraph;
use petgraph::visit::{DfsPostOrder, EdgeRef, Reversed};
use std::cell::{Ref, RefCell, RefMut};
use std::cmp;

#[derive(Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash, Debug)]
/// A unique identifier for nodes in the graph. Stable
/// under graph mutation.
pub struct NodeId(NodeIndex<DefaultIx>);

impl NodeId {
    pub fn input(self, port: u32) -> PortId<InputPort> {
        PortId(self, PortIndex(port, InputPort))
    }
    pub fn output(self, port: u32) -> PortId<OutputPort> {
        PortId(self, PortIndex(port, OutputPort))
    }
}

/// A zero-indexed "port" for a node. Most nodes have one
/// input and one output port, but some may have more.
/// For example, a channel splitter node will have one output
/// port for each channel.
///
/// These are essentially indices into the Chunks
///
/// Kind is a zero sized type and is useful for distinguishing
/// between input and output ports (which may otherwise share indices)
#[derive(Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash, Debug)]
pub struct PortIndex<Kind>(pub u32, pub Kind);

impl<Kind> PortId<Kind> {
    pub fn node(&self) -> NodeId {
        self.0
    }
}

/// An identifier for a port.
#[derive(Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash, Debug)]
pub struct PortId<Kind>(NodeId, PortIndex<Kind>);

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
/// Marker type for denoting that the port is an input port
/// of the node it is connected to
pub struct InputPort;
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
/// Marker type for denoting that the port is an output port
/// of the node it is connected to
pub struct OutputPort;

pub struct AudioGraph {
    graph: StableGraph<Node, Edge>,
    dest_id: NodeId,
}

pub struct Node {
    node: RefCell<Box<AudioNodeEngine>>,
}

/// Edges go *to* the output port from the input port,
///
/// The edge direction is the direction of sound
pub struct Edge {
    /// The index of the port on the input node
    /// This is actually the /output/ of this edge
    input_idx: PortIndex<InputPort>,
    /// The index of the port on the output node
    /// This is actually the /input/ of this edge
    output_idx: PortIndex<OutputPort>,
    /// When the from node finishes processing, it will push
    /// its data into this cache for the input node to read
    cache: RefCell<Option<Block>>,
}

impl AudioGraph {
    pub fn new() -> Self {
        let mut graph = StableGraph::new();
        let dest_id = NodeId(graph.add_node(Node::new(Box::new(DestinationNode::new()))));
        AudioGraph { graph, dest_id }
    }

    /// Create a node, obtain its id
    pub fn add_node(&mut self, node: Box<AudioNodeEngine>) -> NodeId {
        NodeId(self.graph.add_node(Node::new(node)))
    }

    /// Connect an output port to an input port
    ///
    /// The edge goes *from* the output port *to* the input port, connecting two nodes
    pub fn add_edge(&mut self, out: PortId<OutputPort>, inp: PortId<InputPort>) {
        // Output ports can only have a single edge associated with them.
        // Remove all others
        let old = self
            .graph
            .edges(out.node().0)
            .find(|e| e.weight().input_idx == inp.1)
            .map(|e| e.id());
        if let Some(old) = old {
            self.graph.remove_edge(old);
        }
        // add a new edge
        // XXXManishearth it is actually possible for two nodes to have
        // multiple edges between them between
        // different ports. We should represent this somehow.
        self.graph.add_edge(out.node().0, inp.node().0, Edge::new(inp.1, out.1));
    }

    /// Disconnect all outgoing connections from a node
    ///
    /// https://webaudio.github.io/web-audio-api/#dom-audionode-disconnect
    pub fn disconnect_all(&mut self, node: NodeId) {
        let edges = self.graph
                        .edges(node.0)
                        .map(|e| e.id())
                        .collect::<Vec<_>>();
        for edge in edges {
            self.graph.remove_edge(edge);
        }
    }

    /// Disconnect all outgoing connections from a node's output
    ///
    /// https://webaudio.github.io/web-audio-api/#dom-audionode-disconnect-output
    pub fn disconnect_output(&mut self, out: PortId<OutputPort>) {
        // XXXManishearth we don't support multiple connections through
        // a single output yet
        let edge = self
            .graph
            .edges(out.node().0)
            .find(|e| e.weight().output_idx == out.1)
            .map(|e| e.id());
        if let Some(edge) = edge {
            self.graph.remove_edge(edge);
        }
    }

    /// Disconnect connections from a node to another node
    ///
    /// https://webaudio.github.io/web-audio-api/#dom-audionode-disconnect-destinationnode
    pub fn disconnect_between(&mut self, from: NodeId, to: NodeId) {
        let edges = self.graph
                        .edges_directed(to.0, Direction::Incoming)
                        .filter(|e| e.target() == from.0)
                        .map(|e| e.id())
                        .collect::<Vec<_>>();
        for edge in edges {
            self.graph.remove_edge(edge);
        }
    }

    /// Disconnect all outgoing connections from a node's output to another node
    ///
    /// https://webaudio.github.io/web-audio-api/#dom-audionode-disconnect-destinationnode-output
    pub fn disconnect_output_between(&mut self, out: PortId<OutputPort>, to: NodeId) {
        let edge = self
            .graph
            .edges(out.node().0)
            .find(|e| e.weight().output_idx == out.1 && e.source() == to.0)
            .map(|e| e.id());
        if let Some(edge) = edge {
            self.graph.remove_edge(edge);
        }
    }

    /// Disconnect all outgoing connections from a node's output to another node's input
    ///
    /// https://webaudio.github.io/web-audio-api/#dom-audionode-disconnect-destinationnode-output-input
    pub fn disconnect_output_between_to(&mut self, out: PortId<OutputPort>, inp: PortId<InputPort>) {
        let edge = self
            .graph
            .edges(out.node().0)
            .find(|e| e.weight().output_idx == out.1 &&
                      e.source() == inp.node().0 &&
                      e.weight().input_idx == inp.1)
            .map(|e| e.id());
        if let Some(edge) = edge {
            self.graph.remove_edge(edge);
        }
    }

    /// Get the id of the destination node in this graph
    ///
    /// All graphs have a destination node, with one input port
    pub fn dest_id(&self) -> NodeId {
        self.dest_id
    }

    /// For a given block, process all the data on this graph
    pub fn process(&mut self, info: &BlockInfo) -> Chunk {
        // DFS post order: Children are processed before their parent,
        // which is exactly what we need since the parent depends on the
        // children's output
        //
        // This will only visit each node once
        let reversed = Reversed(&self.graph);
        let mut visit = DfsPostOrder::new(reversed, self.dest_id.0);
        while let Some(ix) = visit.next(reversed) {
            let mut curr = self.graph[ix].node.borrow_mut();
            let mut chunk = Chunk::default();
            // if we have inputs, collect all the computed blocks
            // and construct a Chunk
            if curr.input_count() > 0 {
                // set the chunk to the correct size
                chunk
                    .blocks
                    .resize(curr.input_count() as usize, Default::default());

                let mut max = 0; // max channel count
                let mode = curr.channel_count_mode();
                let count = curr.channel_count();
                let interpretation = curr.channel_interpretation();

                // all edges to this node are from its dependencies
                for edge in self.graph.edges_directed(ix, Direction::Incoming) {
                    let edge = edge.weight();
                    // XXXManishearth we can have multiple edges
                    // hitting the same input port, we should deal with that
                    let mut block = edge
                        .cache
                        .borrow_mut()
                        .take()
                        .expect("Cache should have been filled from traversal");
                    if mode == ChannelCountMode::Explicit {
                        block.mix(count, interpretation);
                    } else {
                        max = cmp::max(max, block.chan_count());
                    }

                    chunk[edge.input_idx] = block;
                }

                if mode != ChannelCountMode::Explicit {
                    if mode == ChannelCountMode::ClampedMax {
                        max = cmp::min(max, count);
                    }

                    for block in &mut chunk.blocks {
                        block.mix(max, interpretation);
                    }
                }
            }


            // actually run the node engine
            let mut out = curr.process(chunk, info);

            assert_eq!(out.len(), curr.output_count() as usize);
            if curr.output_count() == 0 {
                continue;
            }

            // all the edges from this node go to nodes which depend on it,
            // i.e. the nodes it outputs to. Store the blocks for retrieval.
            for edge in self.graph.edges(ix) {
                let edge = edge.weight();
                *edge.cache.borrow_mut() = Some(out[edge.output_idx].take());
            }
        }

        // The destination node stores its output on itself, extract it.
        self.graph[self.dest_id.0].node.borrow_mut()
            .destination_data().expect("Destination node should have data cached")
    }


    /// Obtain a mutable reference to a node
    pub fn node_mut(&self, ix: NodeId) -> RefMut<Box<AudioNodeEngine>> {
        self.graph[ix.0].node.borrow_mut()
    }

    /// Obtain an immutable reference to a node
    pub fn node(&self, ix: NodeId) -> Ref<Box<AudioNodeEngine>> {
        self.graph[ix.0].node.borrow()
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

