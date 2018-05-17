use audio::node::AudioNodeEngine;
use audio::oscillator_node::OscillatorNode;
use audio::sink::AudioSink;
use std::sync::mpsc::Receiver;
use std::sync::Arc;

#[cfg(feature = "gst")]
use backends::gstreamer::audio_sink::GStreamerAudioSink;

pub enum AudioGraphMsg {
    ResumeProcessing,
    PauseProcessing,
}

pub struct AudioGraphThread {
    // XXX This should be a graph at some point.
    // It's a single node just for early testing purposes.
    node: Box<AudioNodeEngine>,
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
            // XXX Test with an oscillator node.
            node: Box::new(OscillatorNode::new()),
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

    pub fn event_loop(&self, receiver: Receiver<AudioGraphMsg>) {
        loop {
            if let Ok(msg) = receiver.try_recv() {
                match msg {
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
