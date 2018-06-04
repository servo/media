use audio::block::Chunk;
use audio::node::{AudioNodeEngine, BlockInfo};
use audio::render_thread::AudioRenderThreadMsg;
use audio::sink::AudioSink;
use std::any::Any;
use std::sync::mpsc::Sender;

#[cfg(feature = "gst")]
use backends::gstreamer::audio_sink::GStreamerAudioSink;

pub struct DestinationNode {
    sink: Box<AudioSink>,
}

impl DestinationNode {
    pub fn new() -> Result<Self, ()> {
        #[cfg(feature = "gst")]
        let sink = GStreamerAudioSink::new()?;

        Ok(Self {
            sink: Box::new(sink),
        })
    }

    pub fn init(&self, sample_rate: f32, sender: Sender<AudioRenderThreadMsg>) -> Result<(), ()> {
        self.sink.init(sample_rate, sender)?;
        Ok(self.sink.play())
    }

    pub fn has_enough_data(&self) -> bool {
        self.sink.has_enough_data()
    }
}

impl AudioNodeEngine for DestinationNode {
    fn process(&mut self, inputs: Chunk, _: &BlockInfo) -> Option<Chunk> {
        if !self.sink.push_data(inputs).is_ok() {
            eprintln!("Could not push data to the audio sink");
        }

        // AudioDestinationNodes have no output.
        None
    }

    fn as_any(&self) -> &Any {
        self
    }
}

impl Drop for DestinationNode {
    fn drop(&mut self) {
        self.sink.stop();
    }
}
