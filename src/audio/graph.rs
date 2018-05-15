use audio::node::AudioNodeEngine;
use audio::oscillator_node::OscillatorNode;
use audio::sink::AudioSink;
use std::sync::Arc;

#[cfg(feature = "gst")]
use backends::gstreamer::audio_sink::GStreamerAudioSink;

pub struct AudioGraph {
    // XXX This should be a graph at some point.
    // It's a single node just for early testing purposes.
    node: Box<AudioNodeEngine>,
    sink: Box<AudioSink>,
}

unsafe impl Sync for AudioGraph {}
unsafe impl Send for AudioGraph {}

impl AudioGraph {
    pub fn new() -> Arc<AudioGraph> {
        #[cfg(feature = "gst")]
        let graph = Arc::new(Self {
            // XXX Test with an oscillator node.
            node: Box::new(OscillatorNode::new()),
            sink: Box::new(GStreamerAudioSink::new()),
        });

        let _ = graph.sink.init(graph.clone());

        graph
    }

    pub fn resume_processing(&self) {
        self.sink.play();
    }

    pub fn pause_processing(&self) {
        self.sink.stop();
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
        self.node
            .process(data, accumulator_ref, freq, rate, channels, vol);
    }
}
