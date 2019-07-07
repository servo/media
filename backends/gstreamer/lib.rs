#![feature(nll)]
#![feature(once_is_completed)]

extern crate boxfnonce;
extern crate byte_slice_cast;
extern crate mime;

extern crate glib_sys as glib_ffi;
extern crate gstreamer_sys as gst_ffi;

#[macro_use]
extern crate glib;
#[macro_use]
extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;
extern crate gstreamer_audio as gst_audio;
extern crate gstreamer_base as gst_base;
extern crate gstreamer_player as gst_player;
extern crate gstreamer_sdp as gst_sdp;
extern crate gstreamer_video as gst_video;
extern crate gstreamer_webrtc as gst_webrtc;
extern crate ipc_channel;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

extern crate servo_media;
extern crate servo_media_audio;
extern crate servo_media_gstreamer_render;
extern crate servo_media_player;
extern crate servo_media_streams;
extern crate servo_media_traits;
extern crate servo_media_webrtc;
extern crate url;

pub mod audio_decoder;
pub mod audio_sink;
pub mod media_capture;
pub mod media_stream;
mod media_stream_source;
pub mod player;
mod registry_scanner;
mod render;
mod source;
pub mod webrtc;

use gst::ClockExt;
use ipc_channel::ipc::IpcSender;
use media_stream::GStreamerMediaStream;
use mime::Mime;
use registry_scanner::GSTREAMER_REGISTRY_SCANNER;
use servo_media::{Backend, BackendInit, SupportsMediaType};
use servo_media_audio::context::{AudioContext, AudioContextOptions};
use servo_media_audio::decoder::AudioDecoder;
use servo_media_audio::sink::AudioSinkError;
use servo_media_audio::AudioBackend;
use servo_media_player::context::PlayerGLContext;
use servo_media_player::frame::FrameRenderer;
use servo_media_player::{Player, PlayerEvent, StreamType};
use servo_media_streams::capture::MediaTrackConstraintSet;
use servo_media_streams::registry::MediaStreamId;
use servo_media_streams::MediaOutput;
use servo_media_traits::{ClientContextId, Muteable};
use servo_media_webrtc::{WebRtcBackend, WebRtcController, WebRtcSignaller};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};

lazy_static! {
    static ref BACKEND_BASE_TIME: gst::ClockTime = { gst::SystemClock::obtain().get_time() };
}

struct IdMuteable {
    id: usize,
    muteable: Weak<Mutex<dyn Muteable>>,
}
impl IdMuteable {
    fn new(id: usize, muteable: Weak<Mutex<dyn Muteable>>) -> Self {
        Self { id, muteable }
    }
}
impl PartialEq for IdMuteable {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for IdMuteable {}

impl Hash for IdMuteable {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

pub struct GStreamerBackend {
    capture_mocking: AtomicBool,
    muteables: Mutex<HashMap<ClientContextId, HashSet<IdMuteable>>>,
    next_muteable_id: AtomicUsize,
}

impl GStreamerBackend {
    fn remove_muteable(&self, id: &ClientContextId, muteable_id: usize) {
        let mut muteables = self.muteables.lock().unwrap();
        if let Some(set) = muteables.get_mut(&id) {
            set.retain(|m| m.id != muteable_id);
            if set.len() == 0 {
                muteables.remove(&id);
            }
        }
    }
}

impl Backend for GStreamerBackend {
    fn create_player(
        &self,
        id: &ClientContextId,
        stream_type: StreamType,
        sender: IpcSender<PlayerEvent>,
        renderer: Option<Arc<Mutex<dyn FrameRenderer>>>,
        gl_context: Box<dyn PlayerGLContext>,
    ) -> Arc<Mutex<dyn Player>> {
        let muteable_id = self.next_muteable_id.fetch_add(1, Ordering::Relaxed);
        let player = Arc::new(Mutex::new(player::GStreamerPlayer::new(
            muteable_id,
            stream_type,
            sender,
            renderer,
            gl_context,
        )));
        let mut muteables = self.muteables.lock().unwrap();
        let entry = muteables.entry(*id).or_insert(HashSet::new());
        entry.insert(IdMuteable::new(
            muteable_id,
            Arc::downgrade(&player).clone(),
        ));
        player
    }

    fn shutdown_player(&self, id: &ClientContextId, player: Arc<Mutex<dyn Player>>) {
        let player = player.lock().unwrap();
        let p_id = player.get_id();
        self.remove_muteable(id, p_id);

        if let Err(e) = player.shutdown() {
            warn!("Player was shut down with err: {:?}", e);
        }
    }

    fn create_audio_context(
        &self,
        id: &ClientContextId,
        options: AudioContextOptions,
    ) -> Arc<Mutex<AudioContext>> {
        let muteable_id = self.next_muteable_id.fetch_add(1, Ordering::Relaxed);
        let context = Arc::new(Mutex::new(AudioContext::new::<Self>(muteable_id, options)));
        let mut muteables = self.muteables.lock().unwrap();
        let entry = muteables.entry(*id).or_insert(HashSet::new());
        entry.insert(IdMuteable::new(
            muteable_id,
            Arc::downgrade(&context).clone(),
        ));
        context
    }

    fn shutdown_audio_context(
        &self,
        id: &ClientContextId,
        audio_context: Arc<Mutex<AudioContext>>,
    ) {
        let audio_context = audio_context.lock().unwrap();
        let ac_id = audio_context.get_id();
        self.remove_muteable(id, ac_id);
    }

    fn create_webrtc(&self, signaller: Box<dyn WebRtcSignaller>) -> WebRtcController {
        WebRtcController::new::<Self>(signaller)
    }

    fn create_audiostream(&self) -> MediaStreamId {
        GStreamerMediaStream::create_audio()
    }

    fn create_videostream(&self) -> MediaStreamId {
        GStreamerMediaStream::create_video()
    }

    fn create_stream_output(&self) -> Box<dyn MediaOutput> {
        Box::new(media_stream::MediaSink::new())
    }

    fn create_audioinput_stream(&self, set: MediaTrackConstraintSet) -> Option<MediaStreamId> {
        if self.capture_mocking.load(Ordering::Acquire) {
            // XXXManishearth we should caps filter this
            return Some(self.create_audiostream());
        }
        media_capture::create_audioinput_stream(set)
    }

    fn create_videoinput_stream(&self, set: MediaTrackConstraintSet) -> Option<MediaStreamId> {
        if self.capture_mocking.load(Ordering::Acquire) {
            // XXXManishearth we should caps filter this
            return Some(self.create_videostream());
        }
        media_capture::create_videoinput_stream(set)
    }

    fn can_play_type(&self, media_type: &str) -> SupportsMediaType {
        if let Ok(mime) = media_type.parse::<Mime>() {
            // XXX GStreamer is currently not very reliable playing OGG and most of
            //     the media related WPTs uses OGG if we report that we are able to
            //     play this type. So we report that we are unable to play it to force
            //     the usage of other types.
            //     https://gitlab.freedesktop.org/gstreamer/gst-plugins-base/issues/520
            if mime.subtype() == mime::OGG {
                return SupportsMediaType::No;
            }

            let mime_type = mime.type_().as_str().to_owned() + "/" + mime.subtype().as_str();
            let codecs = match mime.get_param("codecs") {
                Some(codecs) => codecs
                    .as_str()
                    .split(',')
                    .map(|codec| codec.trim())
                    .collect(),
                None => vec![],
            };

            if GSTREAMER_REGISTRY_SCANNER.is_container_type_supported(&mime_type) {
                if codecs.is_empty() {
                    return SupportsMediaType::Maybe;
                } else if GSTREAMER_REGISTRY_SCANNER.are_all_codecs_supported(&codecs) {
                    return SupportsMediaType::Probably;
                } else {
                    return SupportsMediaType::No;
                }
            }
        }
        SupportsMediaType::No
    }

    fn set_capture_mocking(&self, mock: bool) {
        self.capture_mocking.store(mock, Ordering::Release)
    }

    fn mute(&self, id: &ClientContextId, val: bool) {
        let mut muteables = self.muteables.lock().unwrap();
        match muteables.get_mut(id) {
            Some(set) => set.retain(|m_id| {
                if let Some(mutex) = m_id.muteable.upgrade() {
                    let muteable = mutex.lock().unwrap();
                    if muteable.mute(val).is_err() {
                        warn!("Could not mute muteable");
                    }
                    true
                } else {
                    false
                }
            }),
            None => {
                warn!("Trying to mute/unmute an unknown client context");
            }
        }
    }
}

impl AudioBackend for GStreamerBackend {
    type Sink = audio_sink::GStreamerAudioSink;
    fn make_decoder() -> Box<dyn AudioDecoder> {
        Box::new(audio_decoder::GStreamerAudioDecoder::new())
    }
    fn make_sink() -> Result<Self::Sink, AudioSinkError> {
        audio_sink::GStreamerAudioSink::new()
    }
}

impl WebRtcBackend for GStreamerBackend {
    type Controller = webrtc::GStreamerWebRtcController;

    fn construct_webrtc_controller(
        signaller: Box<dyn WebRtcSignaller>,
        thread: WebRtcController,
    ) -> Self::Controller {
        webrtc::construct(signaller, thread).expect("WebRTC creation failed")
    }
}

#[derive(Debug)]
pub struct ErrorLoadingPlugins(Vec<&'static str>);

impl BackendInit for GStreamerBackend {
    fn init() -> Box<dyn Backend> {
        Self::init_with_plugins(PathBuf::new(), &[]).unwrap()
    }
}

impl GStreamerBackend {
    pub fn init_with_plugins(
        plugin_dir: PathBuf,
        plugins: &[&'static str],
    ) -> Result<Box<dyn Backend>, ErrorLoadingPlugins> {
        gst::init().unwrap();

        let mut errors = vec![];
        for plugin in plugins {
            let mut path = plugin_dir.clone();
            path.push(plugin);
            let registry = gst::Registry::get();
            if let Ok(p) = gst::Plugin::load_file(&path) {
                if registry.add_plugin(&p).is_ok() {
                    continue;
                }
            }
            errors.push(*plugin);
        }

        if !errors.is_empty() {
            return Err(ErrorLoadingPlugins(errors));
        }

        Ok(Box::new(GStreamerBackend {
            capture_mocking: AtomicBool::new(false),
            muteables: Mutex::new(HashMap::new()),
            next_muteable_id: AtomicUsize::new(0),
        }))
    }
}

pub fn set_element_flags<T: glib::IsA<gst::Object> + glib::IsA<gst::Element>>(
    element: &T,
    flags: gst::ElementFlags,
) {
    unsafe {
        use glib::translate::ToGlib;

        let ptr: *mut gst_ffi::GstObject = element.as_ptr() as *mut _;
        let _guard = MutexGuard::lock(&(*ptr).lock);
        (*ptr).flags |= flags.to_glib();
    }
}

struct MutexGuard<'a>(&'a glib_ffi::GMutex);

impl<'a> MutexGuard<'a> {
    pub fn lock(mutex: &'a glib_ffi::GMutex) -> Self {
        use glib::translate::mut_override;
        unsafe {
            glib_ffi::g_mutex_lock(mut_override(mutex));
        }
        MutexGuard(mutex)
    }
}

impl<'a> Drop for MutexGuard<'a> {
    fn drop(&mut self) {
        use glib::translate::mut_override;
        unsafe {
            glib_ffi::g_mutex_unlock(mut_override(self.0));
        }
    }
}
