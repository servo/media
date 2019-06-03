pub extern crate servo_media_audio as audio;
pub extern crate servo_media_player as player;
pub extern crate servo_media_streams as streams;
pub extern crate servo_media_webrtc as webrtc;

use std::ops::Deref;
use std::sync::{self, Arc, Mutex, Once};

use audio::context::{AudioContext, AudioContextOptions};
use player::context::PlayerGLContext;
use player::frame::FrameRenderer;
use player::ipc_channel::ipc::IpcSender;
use player::{Player, PlayerEvent, StreamType};
use streams::capture::MediaTrackConstraintSet;
use streams::registry::MediaStreamId;
use streams::MediaOutput;
use webrtc::{WebRtcController, WebRtcSignaller};

pub struct ServoMedia(Box<dyn Backend>);

static INITIALIZER: Once = sync::ONCE_INIT;
static mut INSTANCE: *mut Mutex<Option<Arc<ServoMedia>>> = 0 as *mut _;

pub trait BackendInit {
    fn init() -> Box<dyn Backend>;
}

pub trait Backend: Send + Sync {
    fn create_player(
        &self,
        stream_type: StreamType,
        sender: IpcSender<PlayerEvent>,
        renderer: Option<Arc<Mutex<dyn FrameRenderer>>>,
        gl_context: Box<dyn PlayerGLContext>,
    ) -> Box<dyn Player>;
    fn create_audiostream(&self) -> MediaStreamId;
    fn create_videostream(&self) -> MediaStreamId;
    fn create_stream_output(&self) -> Box<dyn MediaOutput>;
    fn create_audioinput_stream(&self, set: MediaTrackConstraintSet) -> Option<MediaStreamId>;
    fn create_videoinput_stream(&self, set: MediaTrackConstraintSet) -> Option<MediaStreamId>;
    fn create_audio_context(&self, options: AudioContextOptions) -> AudioContext;
    fn create_webrtc(&self, signaller: Box<dyn WebRtcSignaller>) -> WebRtcController;
    fn can_play_type(&self, media_type: &str) -> SupportsMediaType;
    fn set_capture_mocking(&self, _mock: bool) {}
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SupportsMediaType {
    Maybe,
    No,
    Probably,
}

impl ServoMedia {
    pub fn init<B: BackendInit>() {
        INITIALIZER.call_once(|| unsafe {
            let instance = Arc::new(ServoMedia(B::init()));
            INSTANCE = Box::into_raw(Box::new(Mutex::new(Some(instance))));
        })
    }

    pub fn get() -> Result<Arc<ServoMedia>, ()> {
        let instance = unsafe { &*INSTANCE }.lock().unwrap();
        match *instance {
            Some(ref instance) => Ok(instance.clone()),
            None => Err(()),
        }
    }
}

impl Deref for ServoMedia {
    type Target = dyn Backend + 'static;
    fn deref(&self) -> &(dyn Backend + 'static) {
        &*self.0
    }
}
