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
    MetadataUpdated(metadata::Metadata),
    StateChanged(PlaybackState),
    FrameUpdated,
    Error,
}

pub trait Player: Send {
    type Error: Debug;
    fn register_event_handler(&self, sender: IpcSender<PlayerEvent>) -> Result<(), Self::Error>;
    fn register_frame_renderer(&self, renderer: Arc<Mutex<frame::FrameRenderer>>) -> Result<(), Self::Error>;

    fn play(&self) -> Result<(), Self::Error>;
    fn pause(&self) -> Result<(), Self::Error>;
    fn stop(&self) -> Result<(), Self::Error>;

    fn set_input_size(&self, size: u64) -> Result<(), Self::Error>;
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

    fn set_input_size(&self, _: u64) -> Result<(), ()> { Ok(()) }
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
