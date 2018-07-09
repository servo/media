pub mod frame;
pub mod metadata;

use std::sync::mpsc::Sender;

#[derive(Clone, Debug)]
pub enum PlaybackState {
    Stopped,
    // Buffering,
    Paused,
    Playing,
}

#[derive(Clone, Debug)]
pub enum PlayerEvent {
    EndOfStream,
    MetadataUpdated(metadata::Metadata),
    StateChanged(PlaybackState),
    FrameUpdated,
    Error,
}

pub trait Player: Send {
    fn register_event_handler(&self, sender: Sender<PlayerEvent>);
    fn register_frame_renderer(&self, renderer: Box<frame::FrameRenderer>);

    fn setup(&self) -> Result<(), ()>;
    fn play(&self);
    fn stop(&self);

    fn set_input_size(&self, size: u64);
    fn push_data(&self, data: Vec<u8>) -> Result<(), ()>;
    fn end_of_stream(&self) -> Result<(), ()>;
}

pub struct DummyPlayer {}

impl Player for DummyPlayer {
    fn register_event_handler(&self, _: Sender<PlayerEvent>) {}
    fn register_frame_renderer(&self, _: Box<frame::FrameRenderer>) {}

    fn setup(&self) -> Result<(), ()> {
        Err(())
    }
    fn play(&self) {}
    fn stop(&self) {}

    fn set_input_size(&self, _: u64) {}
    fn push_data(&self, _: Vec<u8>) -> Result<(), ()> {
        Err(())
    }
    fn end_of_stream(&self) -> Result<(), ()> {
        Err(())
    }
}

pub trait PlayerBackend {
    type Player: Player;
    fn make_player() -> Result<Self::Player, ()>;
}
