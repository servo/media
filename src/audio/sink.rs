use audio::block::Chunk;
use audio::render_thread::AudioRenderThreadMsg;
use std::sync::mpsc::Sender;

pub trait AudioSink {
    fn init(
        &self,
        sample_rate: f32,
        render_thread_channel: Sender<AudioRenderThreadMsg>,
    ) -> Result<(), ()>;
    fn play(&self);
    fn stop(&self);
    fn has_enough_data(&self) -> bool;
    /// Push a block of audio into the audio sink for playback.
    /// Returns the duration of the audio block in milliseconds.
    fn push_data(&self, chunk: Chunk) -> Result<f64, ()>;
}
