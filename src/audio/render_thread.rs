use audio::block::Chunk;
use audio::destination_node::DestinationNode;
use audio::gain_node::GainNode;
use audio::node::{AudioNodeEngine, AudioNodeMessage, AudioNodeType};
use audio::oscillator_node::OscillatorNode;
use audio::sink::AudioSink;
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

pub enum AudioRenderThreadSyncMsg {
    GetCurrentTime(Sender<f64>),
}

pub struct AudioRenderThread {
    // XXX Test with a hash map for now. This should end up
    // being a graph, like https://docs.rs/petgraph/0.4.12/petgraph/.
    nodes: Vec<Box<AudioNodeEngine>>,
    sink: Box<AudioSink>,
    sample_rate: f32,
    current_time: f64,
}

impl AudioRenderThread {
    pub fn start(
        event_queue: Receiver<AudioRenderThreadMsg>,
        sync_event_queue: Receiver<AudioRenderThreadSyncMsg>,
        sender: Sender<AudioRenderThreadMsg>,
        sample_rate: f32,
    ) -> Result<(), ()> {
        #[cfg(feature = "gst")]
        let sink = GStreamerAudioSink::new()?;

        let mut graph = Self {
            // XXX Test with a vec map for now. This should end up
            // being a graph, like https://docs.rs/petgraph/0.4.12/petgraph/.
            nodes: Vec::new(),
            sink: Box::new(sink),
            sample_rate,
            current_time: 0.,
        };

        graph.sink.init(sample_rate, sender)?;
        graph.event_loop(event_queue, sync_event_queue);

        Ok(())
    }

    fn resume_processing(&self) {
        self.sink.play();
    }

    fn pause_processing(&self) {
        self.sink.stop();
    }

    fn create_node(&mut self, node_type: AudioNodeType) {
        let node: Box<AudioNodeEngine> = match node_type {
            AudioNodeType::OscillatorNode(options) => Box::new(OscillatorNode::new(options)),
            AudioNodeType::DestinationNode => Box::new(DestinationNode::new()),
            AudioNodeType::GainNode(options) => Box::new(GainNode::new(options)),
            _ => unimplemented!(),
        };
        self.nodes.push(node)
    }

    fn process(&self) -> Chunk {
        let mut data = Chunk::default();
        for node in self.nodes.iter() {
            data = node.process(data, self.sample_rate);
        }
        data
    }

    fn event_loop(
        &mut self,
        event_queue: Receiver<AudioRenderThreadMsg>,
        sync_event_queue: Receiver<AudioRenderThreadSyncMsg>,
    ) {
        let handle_msg = move |context: &mut Self, msg: AudioRenderThreadMsg| {
            match msg {
                AudioRenderThreadMsg::CreateNode(node_type) => {
                    context.create_node(node_type);
                }
                AudioRenderThreadMsg::ResumeProcessing => {
                    context.resume_processing();
                }
                AudioRenderThreadMsg::PauseProcessing => {
                    context.pause_processing();
                }
                AudioRenderThreadMsg::MessageNode(index, msg) => {
                    context.nodes[index].message(msg)
                }
                AudioRenderThreadMsg::SinkNeedData => {
                    // Do nothing. This will simply unblock the thread so we
                    // can restart the non-blocking event loop.
                }
            };
        };

        let handle_sync_msg = move |context: &Self, msg: AudioRenderThreadSyncMsg| {
            match msg {
                AudioRenderThreadSyncMsg::GetCurrentTime(response) => {
                    response.send(context.current_time).unwrap()
                }
            };
        };

        loop {
            if self.sink.has_enough_data() {
                // If we have already pushed enough data into the audio sink
                // we wait for messages coming from the control thread or
                // the audio sink. The audio sink will notify whenever it
                // needs more data.
                if let Ok(msg) = event_queue.recv() {
                    handle_msg(self, msg);
                }
            } else {
                // If we have not pushed enough data into the audio sink yet,
                // we process the control message queue
                if let Ok(msg) = event_queue.try_recv() {
                    handle_msg(self, msg);
                }
                // and push into the audio sink the result of processing a
                // render quantum.
                if let Ok(duration) = self.sink.push_data(self.process()) {
                    self.current_time += duration;
                } else {
                    eprintln!("Could not push data to audio sink");
                }
            }

            if let Ok(msg) = sync_event_queue.try_recv() {
                handle_sync_msg(self, msg);
            }
        }
    }
}
