use audio::block::Chunk;
use audio::destination_node::DestinationNode;
use audio::gain_node::GainNode;
use audio::node::{AudioNodeEngine, AudioNodeType, AudioNodeMessage};
use audio::oscillator_node::OscillatorNode;
use audio::sink::AudioSink;
use std::cell::RefCell;
use std::sync::mpsc::Receiver;
use std::sync::Arc;

#[cfg(feature = "gst")]
use backends::gstreamer::audio_sink::GStreamerAudioSink;

pub enum AudioGraphThreadMsg {
    CreateNode(AudioNodeType),
    MessageNode(usize, AudioNodeMessage),
    ResumeProcessing,
    PauseProcessing,
}

pub struct AudioGraphThread {
    // XXX Test with a hash map for now. This should end up
    // being a graph, like https://docs.rs/petgraph/0.4.12/petgraph/.
    nodes: RefCell<Vec<Box<AudioNodeEngine>>>,
    sink: Box<AudioSink>,
}

// XXX This is only required until we update gstreamer
// https://github.com/sdroege/gstreamer-rs/commit/062403bdacf0658b719731bc38b570dcf500366e#diff-8fec33a7daa25b45af418d646ff7ea24
unsafe impl Sync for AudioGraphThread {}
unsafe impl Send for AudioGraphThread {}

impl AudioGraphThread {
    pub fn start(event_queue: Receiver<AudioGraphThreadMsg>) {
        #[cfg(feature = "gst")]
        let graph = Arc::new(Self {
            // XXX Test with a vec map for now. This should end up
            // being a graph, like https://docs.rs/petgraph/0.4.12/petgraph/.
            nodes: RefCell::new(Vec::new()),
            sink: Box::new(GStreamerAudioSink::new()),
        });

        let _ = graph.sink.init(graph.clone());

        graph.event_loop(event_queue);
    }

    pub fn resume_processing(&self) {
        self.sink.play();
    }

    pub fn pause_processing(&self) {
        self.sink.stop();
    }

    pub fn create_node(&self, node_type: AudioNodeType) {
        let node: Box<AudioNodeEngine> = match node_type {
            AudioNodeType::OscillatorNode(options) => Box::new(OscillatorNode::new(options)),
            AudioNodeType::DestinationNode => Box::new(DestinationNode::new()),
            AudioNodeType::GainNode(options) => Box::new(GainNode::new(options)),
            _ => unimplemented!(),
        };
        let mut nodes = self.nodes.borrow_mut();
        nodes.push(node)
    }

    pub fn process(&self, rate: u32) -> Chunk {
        let nodes = self.nodes.borrow();
        let mut data = Chunk::default();
        for node in nodes.iter() {
            data = node.process(data, rate);
        }
        data
    }

    pub fn event_loop(&self, event_queue: Receiver<AudioGraphThreadMsg>) {
        loop {
            if let Ok(msg) = event_queue.try_recv() {
                match msg {
                    AudioGraphThreadMsg::CreateNode(node_type) => {
                        self.create_node(node_type);
                    }
                    AudioGraphThreadMsg::ResumeProcessing => {
                        self.resume_processing();
                    }
                    AudioGraphThreadMsg::PauseProcessing => {
                        self.pause_processing();
                    }
                    AudioGraphThreadMsg::MessageNode(index, msg) => {
                        self.nodes.borrow_mut()[index].message(msg)
                    }
                }
            }
        }
    }
}
