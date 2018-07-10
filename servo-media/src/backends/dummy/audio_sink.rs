use audio::block::Chunk;
use audio::render_thread::AudioRenderThreadMsg;
use audio::sink::AudioSink;
use std::sync::mpsc::Sender;

pub struct DummyAudioSink {}

impl AudioSink for DummyAudioSink {
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
