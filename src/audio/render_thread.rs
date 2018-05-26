use audio::block::Chunk;
use audio::destination_node::DestinationNode;
use audio::gain_node::GainNode;
use audio::node::{AudioNodeEngine, AudioNodeMessage, AudioNodeType};
use audio::oscillator_node::OscillatorNode;
use audio::sink::AudioSink;
use std::cell::RefCell;
use std::sync::mpsc::{Receiver, Sender};

#[cfg(feature = "gst")]
use backends::gstreamer::audio_sink::GStreamerAudioSink;

pub enum AudioRenderThreadMsg {
    CreateNode(AudioNodeType),
    MessageNode(usize, AudioNodeMessage),
    ResumeProcessing,
    PauseProcessing,
    SinkNeedData,
}

pub struct AudioRenderThread {
    // XXX Test with a hash map for now. This should end up
    // being a graph, like https://docs.rs/petgraph/0.4.12/petgraph/.
    nodes: RefCell<Vec<Box<AudioNodeEngine>>>,
    sink: Box<AudioSink>,
    sample_rate: f32,
}

impl AudioRenderThread {
    pub fn start(
        event_queue: Receiver<AudioRenderThreadMsg>,
        sender: Sender<AudioRenderThreadMsg>,
    ) -> Result<(), ()> {
        #[cfg(feature = "gst")]
        let sink = GStreamerAudioSink::new()?;

        let graph = Self {
            // XXX Test with a vec map for now. This should end up
            // being a graph, like https://docs.rs/petgraph/0.4.12/petgraph/.
            nodes: RefCell::new(Vec::new()),
            sink: Box::new(sink),
            // XXX Get this from AudioContextOptions.
            sample_rate: 44100.,
        };

        graph.sink.init(graph.sample_rate, sender)?;

        graph.event_loop(event_queue);

        Ok(())
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

    pub fn process(&self) -> Chunk {
        let nodes = self.nodes.borrow();
        let mut data = Chunk::default();
        for node in nodes.iter() {
            data = node.process(data, self.sample_rate);
        }
        data
    }

    pub fn event_loop(&self, event_queue: Receiver<AudioRenderThreadMsg>) {
        let handle_msg = move |msg: AudioRenderThreadMsg| {
            match msg {
                AudioRenderThreadMsg::CreateNode(node_type) => {
                    self.create_node(node_type);
                }
                AudioRenderThreadMsg::ResumeProcessing => {
                    self.resume_processing();
                }
                AudioRenderThreadMsg::PauseProcessing => {
                    self.pause_processing();
                }
                AudioRenderThreadMsg::MessageNode(index, msg) => {
                    self.nodes.borrow_mut()[index].message(msg)
                }
                AudioRenderThreadMsg::SinkNeedData => {
                    // Do nothing. This will simply unblock the thread so we
                    // can restart the non-blocking event loop.
                }
            }
        };

        loop {
            if self.sink.has_enough_data() {
                // If we have already pushed enough data into the audio sink
                // we wait for messages coming from the control thread or
                // the audio sink. The audio sink will notify whenever it
                // needs more data.
                if let Ok(msg) = event_queue.recv() {
                    handle_msg(msg);
                }
            } else {
                // If we have not pushed enough data into the audio sink yet,
                // we process the control message queue
                if let Ok(msg) = event_queue.try_recv() {
                    handle_msg(msg);
                }
                // and push into the audio sink the result of processing a
                // render quantum.
                let _ = self.sink.push_data(self.process());
            }
        }
    }
}
