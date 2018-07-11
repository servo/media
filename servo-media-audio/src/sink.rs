use block::Chunk;
use render_thread::AudioRenderThreadMsg;
use std::sync::mpsc::Sender;

pub trait AudioSink {
    fn init(
        &self,
        sample_rate: f32,
        render_thread_channel: Sender<AudioRenderThreadMsg>,
    ) -> Result<(), ()>;
    fn play(&self) -> Result<(), ()>;
    fn stop(&self) -> Result<(), ()>;
    fn has_enough_data(&self) -> bool;
    fn push_data(&self, chunk: Chunk) -> Result<(), ()>;
}
