use block::Chunk;
use render_thread::AudioRenderThreadMsg;
use sink::AudioSink;
use std::sync::mpsc::Sender;

pub struct OfflineAudioContext {
}

impl AudioSink for OfflineAudioContext {
    fn init(&self, _: f32, _: Sender<AudioRenderThreadMsg>) -> Result<(), ()> {
        Ok(())
    }
    fn play(&self) -> Result<(), ()> {
        Ok(())
    }
    fn stop(&self) -> Result<(), ()> {
        Ok(())
    }
    fn has_enough_data(&self) -> bool {
        true
    }
    fn push_data(&self, _: Chunk) -> Result<(), ()> {
        Ok(())
    }
}
