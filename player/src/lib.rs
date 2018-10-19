extern crate ipc_channel;
#[macro_use]
extern crate serde_derive;

pub mod frame;
pub mod metadata;

use ipc_channel::ipc::IpcSender;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum PlaybackState {
    Stopped,
    // Buffering,
    Paused,
    Playing,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum PlayerEvent {
    EndOfStream,
    Error,
    FrameUpdated,
    MetadataUpdated(metadata::Metadata),
    PositionChanged(u64),
    /// The player needs the data to perform a seek to the given offset.
    /// The next push_data should get the buffers from the new offset.
    /// This event is only received for seekable stream types.
    SeekData(u64),
    /// The player has performed a seek to the given offset.
    SeekDone(u64),
    StateChanged(PlaybackState),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum StreamType {
    /// No seeking is supported in the stream, such as a live stream.
    NonSeekable,
    /// The stream is seekable but seeking might not be very fast, such as data from a webserver.
    Seekable,
    /// The stream is seekable and seeking is fast, such as in a local file.
    SeekableFast,
}

pub trait Player: Send {
    type Error: Debug;
    fn register_event_handler(&self, sender: IpcSender<PlayerEvent>) -> Result<(), Self::Error>;
    fn register_frame_renderer(&self, renderer: Arc<Mutex<frame::FrameRenderer>>) -> Result<(), Self::Error>;

    fn play(&self) -> Result<(), Self::Error>;
    fn pause(&self) -> Result<(), Self::Error>;
    fn stop(&self) -> Result<(), Self::Error>;
    fn seek(&self, time: f64) -> Result<(), Self::Error>;

    fn set_input_size(&self, size: u64) -> Result<(), Self::Error>;
    fn set_stream_type(&self, type_: StreamType) -> Result<(), Self::Error>;
    fn push_data(&self, data: Vec<u8>) -> Result<(), Self::Error>;
    fn end_of_stream(&self) -> Result<(), Self::Error>;
}

pub struct DummyPlayer {}

impl Player for DummyPlayer {
    type Error = ();
    fn register_event_handler(&self, _: IpcSender<PlayerEvent>) -> Result<(), ()> { Ok(()) }
    fn register_frame_renderer(&self, _: Arc<Mutex<frame::FrameRenderer>>) -> Result<(), ()> { Ok(()) }

    fn play(&self) -> Result<(), ()> { Ok(()) }
    fn pause(&self) -> Result<(), ()> { Ok(()) }
    fn stop(&self) -> Result<(), ()> { Ok(()) }
    fn seek(&self, _: f64) -> Result<(), ()> { Ok(()) }

    fn set_input_size(&self, _: u64) -> Result<(), ()> { Ok(()) }
    fn set_stream_type(&self, _: StreamType) -> Result<(), ()> { Ok(()) }
    fn push_data(&self, _: Vec<u8>) -> Result<(), ()> {
        Err(())
    }
    fn end_of_stream(&self) -> Result<(), ()> {
        Err(())
    }
}

pub trait PlayerBackend {
    type Player: Player;
    fn make_player() -> Self::Player;
}
