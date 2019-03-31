use block::Chunk;
use render_thread::AudioRenderThreadMsg;
use std::sync::mpsc::Sender;

#[derive(Debug, PartialEq)]
pub enum AudioSinkError {
    /// Backend specific error.
    Backend(String),
    /// Could not push buffer into the audio sink.
    BufferPushFailed,
    /// Could not move to a different state.
    StateChangeFailed,
}

pub trait AudioSink {
    fn init(
        &self,
        sample_rate: f32,
        render_thread_channel: Sender<AudioRenderThreadMsg>,
    ) -> Result<(), AudioSinkError>;
    fn play(&self) -> Result<(), AudioSinkError>;
    fn stop(&self) -> Result<(), AudioSinkError>;
    fn has_enough_data(&self) -> bool;
    fn push_data(&self, chunk: Chunk) -> Result<(), AudioSinkError>;
    fn set_eos_callback(&self, callback: Box<Fn(Box<AsRef<[f32]>>) + Send + Sync + 'static>);
}
