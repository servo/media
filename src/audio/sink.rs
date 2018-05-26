use audio::block::Chunk;

pub trait AudioSink {
    fn init(&mut self, rate: u32) -> Result<(), ()>;
    fn play(&self);
    fn stop(&self);
    fn send_chunk(&self, chunk: Chunk);
}
