use audio::block::Chunk;

pub trait AudioSink {
    fn init(&self, sample_rate: f32) -> Result<(), ()>;
    fn play(&self);
    fn stop(&self);
    fn has_enough_data(&self) -> bool;
    fn push_data(&self, chunk: Chunk) -> Result<(), ()>;
}
