use audio::block::Chunk;
use audio::render_thread::AudioRenderThreadMsg;
use std::sync::mpsc::Sender;

pub trait AudioSink: Send {
    fn init(
        &self,
        sample_rate: f32,
        render_thread_channel: Sender<AudioRenderThreadMsg>,
    ) -> Result<(), ()>;
    fn play(&self);
    fn stop(&self);
    fn has_enough_data(&self) -> bool;
    fn push_data(&self, chunk: Chunk) -> Result<(), ()>;
}
