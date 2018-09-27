use block::Chunk;
use render_thread::AudioRenderThreadMsg;
use std::fmt::Debug;
use std::sync::mpsc::Sender;

pub trait AudioSink {
    type Error: Debug;
    fn init(
        &self,
        sample_rate: f32,
        render_thread_channel: Sender<AudioRenderThreadMsg>,
    ) -> Result<(), Self::Error>;
    fn play(&self) -> Result<(), Self::Error>;
    fn stop(&self) -> Result<(), Self::Error>;
    fn has_enough_data(&self) -> bool;
    fn push_data(&self, chunk: Chunk) -> Result<(), Self::Error>;
    fn set_eos_callback(&self, callback: Box<Fn(Box<AsRef<[f32]>>) + Send + Sync + 'static>);
}

pub struct DummyAudioSink;

impl AudioSink for DummyAudioSink {
    type Error = ();
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
    fn set_eos_callback(&self, _: Box<Fn(Box<AsRef<[f32]>>) + Send + Sync + 'static>) {}
}
