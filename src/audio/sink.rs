use audio::block::Chunk;
use audio::graph_thread::AudioGraphThreadMsg;
use std::sync::mpsc::Sender;

pub trait AudioSink {
    fn init(
        &self,
        sample_rate: f32,
        graph_thread_channel: Sender<AudioGraphThreadMsg>,
    ) -> Result<(), ()>;
    fn play(&self);
    fn stop(&self);
    fn has_enough_data(&self) -> bool;
    fn push_data(&self, chunk: Chunk) -> Result<(), ()>;
}
