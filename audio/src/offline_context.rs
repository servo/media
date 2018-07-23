use block::Chunk;
use render_thread::AudioRenderThreadMsg;
use sink::AudioSink;
use std::cell::Cell;
use std::sync::mpsc::Sender;

pub struct OfflineAudioContext {
    has_enough_data: Cell<bool>,
    length: usize,
    rendered_bytes: Cell<usize>,
}

impl OfflineAudioContext {
    pub fn new(length: usize) -> Self {
        Self {
            has_enough_data: Cell::new(false),
            length,
            rendered_bytes: Cell::new(0),
        }
    }
}

impl AudioSink for OfflineAudioContext {
    fn init(&self, _: f32, _: Sender<AudioRenderThreadMsg>) -> Result<(), ()> {
        Ok(())
    }
    fn play(&self) -> Result<(), ()> {
        self.has_enough_data.set(false);
        Ok(())
    }
    fn stop(&self) -> Result<(), ()> {
        self.has_enough_data.set(true);
        Ok(())
    }
    fn has_enough_data(&self) -> bool {
        self.has_enough_data.get() && (self.rendered_bytes.get() < self.length)
    }
    fn push_data(&self, chunk: Chunk) -> Result<(), ()> {
        self.rendered_bytes.update(|bytes| bytes + chunk.len());
        Ok(())
    }
}
