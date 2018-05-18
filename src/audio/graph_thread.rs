use audio::node::{AudioNodeEngine, AudioNodeType};
use audio::oscillator_node::OscillatorNode;
use audio::sink::AudioSink;
use std::cell::RefCell;
use std::collections::hash_map::HashMap;
use std::sync::mpsc::Receiver;
use std::sync::Arc;

#[cfg(feature = "gst")]
use backends::gstreamer::audio_sink::GStreamerAudioSink;

pub enum AudioGraphMsg {
    CreateNode(usize, AudioNodeType),
    ResumeProcessing,
    PauseProcessing,
}

pub struct AudioGraphThread {
    // XXX Test with a hash map for now. This should end up
    // being a graph, like https://docs.rs/petgraph/0.4.12/petgraph/.
    nodes: RefCell<HashMap<usize, Box<AudioNodeEngine>>>,
    sink: Box<AudioSink>,
}

// XXX This is only required until we update gstreamer
// https://github.com/sdroege/gstreamer-rs/commit/062403bdacf0658b719731bc38b570dcf500366e#diff-8fec33a7daa25b45af418d646ff7ea24
unsafe impl Sync for AudioGraphThread {}
unsafe impl Send for AudioGraphThread {}

impl AudioGraphThread {
    pub fn start(receiver: Receiver<AudioGraphMsg>) {
        #[cfg(feature = "gst")]
        let graph = Arc::new(Self {
            // XXX Test with a hash map for now. This should end up
            // being a graph, like https://docs.rs/petgraph/0.4.12/petgraph/.
            nodes: RefCell::new(HashMap::new()),
            sink: Box::new(GStreamerAudioSink::new()),
        });

        let _ = graph.sink.init(graph.clone());

        graph.event_loop(receiver);
    }

    pub fn resume_processing(&self) {
        self.sink.play();
    }

    pub fn pause_processing(&self) {
        self.sink.stop();
    }

    pub fn create_node(&self, node_id: usize, node_type: AudioNodeType) {
        match node_type {
            AudioNodeType::OscillatorNode(options) => {
                let node = Box::new(OscillatorNode::new(options));
                let mut nodes = self.nodes.borrow_mut();
                nodes.insert(node_id, node);
            }
            _ => unimplemented!(),
        }
    }

    pub fn process(
        &self,
        data: &mut [u8],
        accumulator_ref: &mut f64,
        freq: u32,
        rate: u32,
        channels: u32,
        vol: f64,
    ) {
        let nodes = self.nodes.borrow();
        for (_, node) in nodes.iter() {
            node.process(data, accumulator_ref, freq, rate, channels, vol);
        }
    }

    pub fn event_loop(&self, receiver: Receiver<AudioGraphMsg>) {
        loop {
            if let Ok(msg) = receiver.try_recv() {
                match msg {
                    AudioGraphMsg::CreateNode(node_id, node_type) => {
                        self.create_node(node_id, node_type);
                    }
                    AudioGraphMsg::ResumeProcessing => {
                        self.resume_processing();
                    }
                    AudioGraphMsg::PauseProcessing => {
                        self.pause_processing();
                    }
                }
            }
        }
    }
}
