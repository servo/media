use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, Once};
use std::time;

use super::BACKEND_BASE_TIME;
use crate::media_stream::GStreamerMediaStream;
use crate::media_stream_source::{ServoMediaStreamSrc, register_servo_media_stream_src};
use crate::render::GStreamerRender;
use crate::source::{ServoSrc, register_servo_src};
use byte_slice_cast::AsSliceOf;
use glib;
use glib::prelude::*;
use gst;
use gst::prelude::*;
use gst_app;
use gst_play;
use gst_play::prelude::*;
use ipc_channel::ipc::{IpcReceiver, IpcSender, channel};
use servo_media_player::audio::AudioRenderer;
use servo_media_player::context::PlayerGLContext;
use servo_media_player::metadata::Metadata;
use servo_media_player::video::VideoFrameRenderer;
use servo_media_player::{
    PlaybackState, Player, PlayerError, PlayerEvent, SeekLock, SeekLockMsg, StreamType,
};
use servo_media_streams::registry::{MediaStreamId, get_stream};
use servo_media_traits::{BackendMsg, ClientContextId, MediaInstance};

const DEFAULT_MUTED: bool = false;
const DEFAULT_PAUSED: bool = true;
const DEFAULT_PLAYBACK_RATE: f64 = 1.0;
const DEFAULT_VOLUME: f64 = 1.0;
const DEFAULT_TIME_RANGES: Vec<Range<f64>> = vec![];

const MAX_BUFFER_SIZE: i32 = 500 * 1024 * 1024;

fn metadata_from_media_info(media_info: &gst_play::PlayMediaInfo) -> Result<Metadata, ()> {
    let dur = media_info.duration();
    let duration = if let Some(dur) = dur {
        let mut nanos = dur.nseconds();
        nanos = nanos % 1_000_000_000;
        let seconds = dur.seconds();
        Some(time::Duration::new(seconds, nanos as u32))
    } else {
        None
    };

    let mut audio_tracks = Vec::new();
    let mut video_tracks = Vec::new();

    let format = media_info
        .container_format()
        .unwrap_or_else(|| glib::GString::from(""))
        .to_string();

    for stream_info in media_info.stream_list() {
        let stream_type = stream_info.stream_type();
        match stream_type.as_str() {
            "audio" => {
                let codec = stream_info
                    .codec()
                    .unwrap_or_else(|| glib::GString::from(""))
                    .to_string();
                audio_tracks.push(codec);
            },
            "video" => {
                let codec = stream_info
                    .codec()
                    .unwrap_or_else(|| glib::GString::from(""))
                    .to_string();
                video_tracks.push(codec);
            },
            _ => {},
        }
    }

    let mut width: u32 = 0;
    let height: u32 = if media_info.number_of_video_streams() > 0 {
        let first_video_stream = &media_info.video_streams()[0];
        width = first_video_stream.width() as u32;
        first_video_stream.height() as u32
    } else {
        0
    };

    let is_seekable = media_info.is_seekable();
    let is_live = media_info.is_live();
    let title = media_info.title().map(|s| s.as_str().to_string());

    Ok(Metadata {
        duration,
        width,
        height,
        format,
        is_seekable,
        audio_tracks,
        video_tracks,
        is_live,
        title,
    })
}

pub struct GStreamerAudioChunk(gst::buffer::MappedBuffer<gst::buffer::Readable>);
impl AsRef<[f32]> for GStreamerAudioChunk {
    fn as_ref(&self) -> &[f32] {
        self.0.as_ref().as_slice_of::<f32>().unwrap_or_default()
    }
}

#[derive(PartialEq)]
enum PlayerSource {
    Seekable(ServoSrc),
    Stream(ServoMediaStreamSrc),
}

struct SharedState {
    category: gst::DebugCategory,
    // Track `play` state to send expected `paused` state change event.
    // TODO: <https://github.com/servo/servo/issues/40740>
    play_state: gst_play::PlayState,
    pending_input_size: Option<u64>,
    // The playback rate will not be passed to the pipeline if the current
    // GstPlay state is less than GST_STATE_PAUSED.
    pending_playback_rate: Option<f64>,
    metadata: Option<Metadata>,
}

struct PlayerInner {
    player: gst_play::Play,
    _signal_adapter: gst_play::PlaySignalAdapter,
    source: Option<PlayerSource>,
    video_sink: gst_app::AppSink,
    input_size: Cell<u64>,
    paused: Cell<bool>,
    playback_rate: Cell<f64>,
    muted: Cell<bool>,
    volume: Cell<f64>,
    stream_type: StreamType,
    cat: gst::DebugCategory,
    enough_data: Arc<AtomicBool>,
    shared_state: Arc<Mutex<SharedState>>,
}

impl PlayerInner {
    pub fn set_input_size(&mut self, size: u64) -> Result<(), PlayerError> {
        // Set input_size to proxy its value, since it could be set by the user
        // before calling .setup().
        if self.input_size.get() == size {
            return Ok(());
        }

        self.input_size.set(size);

        match self.source {
            // The input size is only useful for seekable streams.
            Some(ref mut source) => {
                if let PlayerSource::Seekable(source) = source {
                    source.set_size(if size > 0 {
                        size as i64
                    } else {
                        -1 // live source
                    });
                }
            },
            _ => {
                self.shared_state.lock().unwrap().pending_input_size = Some(size);
            },
        }
        Ok(())
    }

    pub fn set_mute(&mut self, muted: bool) -> Result<(), PlayerError> {
        if self.muted.get() == muted {
            return Ok(());
        }

        self.muted.set(muted);
        self.player.set_mute(muted);
        Ok(())
    }

    pub fn muted(&self) -> bool {
        self.muted.get()
    }

    pub fn set_playback_rate(&mut self, playback_rate: f64) -> Result<(), PlayerError> {
        if self.stream_type != StreamType::Seekable {
            return Err(PlayerError::NonSeekableStream);
        }

        if self.playback_rate.get() == playback_rate {
            return Ok(());
        }

        self.playback_rate.set(playback_rate);

        // The new playback rate will not be passed to the pipeline if the
        // current GstPlay state is less than GST_STATE_PAUSED, which will be
        // set immediately before the initial GST_PLAY_MESSAGE_MEDIA_INFO_UPDATED
        // message is posted to bus.
        let mut shared_state = self.shared_state.lock().unwrap();
        if shared_state.metadata.is_some() {
            self.player.set_rate(playback_rate);
        } else {
            shared_state.pending_playback_rate = Some(playback_rate);
        }

        Ok(())
    }

    pub fn playback_rate(&self) -> f64 {
        self.playback_rate.get()
    }

    pub fn play(&mut self) -> Result<(), PlayerError> {
        if !self.paused.get() {
            return Ok(());
        }

        self.paused.set(false);
        self.player.play();
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), PlayerError> {
        self.player.stop();
        self.paused.set(true);
        self.shared_state.lock().unwrap().metadata = None;
        if let Some(source) = self.source.take() {
            if let PlayerSource::Seekable(source) = source {
                source.set_callbacks(gst_app::AppSrcCallbacks::builder().build());
            }
        }
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), PlayerError> {
        if self.paused.get() {
            return Ok(());
        }

        self.paused.set(true);
        self.player.pause();
        Ok(())
    }

    pub fn paused(&self) -> bool {
        self.paused.get()
    }

    pub fn end_of_stream(&mut self) -> Result<(), PlayerError> {
        match self.source {
            Some(ref mut source) => {
                if let PlayerSource::Seekable(source) = source {
                    source
                        .push_end_of_stream()
                        .map(|_| ())
                        .map_err(|_| PlayerError::EOSFailed)
                } else {
                    Ok(())
                }
            },
            _ => Ok(()),
        }
    }

    pub fn seek(&mut self, time: f64) -> Result<(), PlayerError> {
        if self.stream_type != StreamType::Seekable {
            return Err(PlayerError::NonSeekableStream);
        }

        if let Some(ref duration) = self
            .shared_state
            .lock()
            .unwrap()
            .metadata
            .as_ref()
            .map(|metadata| metadata.duration)
            .flatten()
        {
            if duration < &time::Duration::new(time as u64, 0) {
                gst::warning!(self.cat, obj = &self.player, "Trying to seek out of range");
                return Err(PlayerError::SeekOutOfRange);
            }
        }

        let time = time * 1_000_000_000.;
        self.player.seek(gst::ClockTime::from_nseconds(time as u64));
        Ok(())
    }

    pub fn set_volume(&mut self, volume: f64) -> Result<(), PlayerError> {
        if self.volume.get() == volume {
            return Ok(());
        }

        self.volume.set(volume);
        self.player.set_volume(volume);
        Ok(())
    }

    pub fn volume(&self) -> f64 {
        self.volume.get()
    }

    pub fn push_data(&mut self, data: Vec<u8>) -> Result<(), PlayerError> {
        if let Some(ref mut source) = self.source {
            if let PlayerSource::Seekable(source) = source {
                if self.enough_data.load(Ordering::Relaxed) {
                    return Err(PlayerError::EnoughData);
                }
                return source
                    .push_buffer(data)
                    .map(|_| ())
                    .map_err(|_| PlayerError::BufferPushFailed);
            }
        }
        Err(PlayerError::BufferPushFailed)
    }

    pub fn set_src(&mut self, source: PlayerSource) {
        self.source = Some(source);
    }

    pub fn buffered(&self) -> Vec<Range<f64>> {
        let mut buffered_ranges = vec![];

        let Some(duration) = self
            .shared_state
            .lock()
            .unwrap()
            .metadata
            .as_ref()
            .map(|metadata| metadata.duration)
            .flatten()
        else {
            return buffered_ranges;
        };

        let pipeline = self.player.pipeline();
        let mut buffering = gst::query::Buffering::new(gst::Format::Percent);
        if pipeline.query(&mut buffering) {
            let ranges = buffering.ranges();
            for (start, end) in ranges {
                let start = (if let gst::GenericFormattedValue::Percent(start) = start {
                    start.unwrap()
                } else {
                    gst::format::Percent::from_percent(0)
                } / gst::format::Percent::MAX) as f64
                    * duration.as_secs_f64();
                let end = (if let gst::GenericFormattedValue::Percent(end) = end {
                    end.unwrap()
                } else {
                    gst::format::Percent::from_percent(0)
                } / gst::format::Percent::MAX) as f64
                    * duration.as_secs_f64();
                buffered_ranges.push(Range { start, end });
            }
        }

        buffered_ranges
    }

    pub fn seekable(&self) -> Vec<Range<f64>> {
        // if the servosrc is seekable, we should return the duration of the media
        if let Some(ref metadata) = self
            .shared_state
            .lock()
            .unwrap()
            .metadata
            .as_ref()
            .filter(|metadata| metadata.is_seekable)
        {
            if let Some(duration) = metadata.duration {
                return vec![Range {
                    start: 0.0,
                    end: duration.as_secs_f64(),
                }];
            }
        }

        // if the servosrc is not seekable, we should return the buffered range
        self.buffered()
    }

    fn set_stream(&mut self, stream: &MediaStreamId, only_stream: bool) -> Result<(), PlayerError> {
        debug_assert!(self.stream_type == StreamType::Stream);
        if let Some(ref source) = self.source {
            if let PlayerSource::Stream(source) = source {
                let stream =
                    get_stream(stream).expect("Media streams registry does not contain such ID");
                let mut stream = stream.lock().unwrap();
                if let Some(mut stream) = stream.as_mut_any().downcast_mut::<GStreamerMediaStream>()
                {
                    let playbin = self
                        .player
                        .pipeline()
                        .dynamic_cast::<gst::Pipeline>()
                        .unwrap();
                    let clock = gst::SystemClock::obtain();
                    playbin.set_base_time(*BACKEND_BASE_TIME);
                    playbin.set_start_time(gst::ClockTime::NONE);
                    playbin.use_clock(Some(&clock));

                    source.set_stream(&mut stream, only_stream);
                    return Ok(());
                }
            }
        }
        Err(PlayerError::SetStreamFailed)
    }

    fn set_audio_track(&mut self, stream_index: i32, enabled: bool) -> Result<(), PlayerError> {
        self.player
            .set_audio_track(stream_index)
            .map_err(|_| PlayerError::SetTrackFailed)?;
        self.player.set_audio_track_enabled(enabled);
        Ok(())
    }

    fn set_video_track(&mut self, stream_index: i32, enabled: bool) -> Result<(), PlayerError> {
        self.player
            .set_video_track(stream_index)
            .map_err(|_| PlayerError::SetTrackFailed)?;
        self.player.set_video_track_enabled(enabled);
        Ok(())
    }
}

macro_rules! notify(
    ($observer:expr_2021, $event:expr_2021) => {
        $observer.lock().unwrap().send($event)
    };
);

struct SeekChannel {
    sender: SeekLock,
    recv: IpcReceiver<SeekLockMsg>,
}

impl SeekChannel {
    fn new() -> Self {
        let (sender, recv) = channel::<SeekLockMsg>().expect("Couldn't create IPC channel");
        Self {
            sender: SeekLock {
                lock_channel: sender,
            },
            recv,
        }
    }

    fn sender(&self) -> SeekLock {
        self.sender.clone()
    }

    fn _await(&self) -> SeekLockMsg {
        self.recv.recv().unwrap()
    }
}

pub struct GStreamerPlayer {
    /// The player unique ID.
    id: usize,
    /// The ID of the client context this player belongs to.
    context_id: ClientContextId,
    /// Channel to communicate with the owner GStreamerBackend instance.
    backend_chan: Arc<Mutex<Sender<BackendMsg>>>,
    inner: RefCell<Option<Arc<Mutex<PlayerInner>>>>,
    observer: Arc<Mutex<IpcSender<PlayerEvent>>>,
    audio_renderer: Option<Arc<Mutex<dyn AudioRenderer>>>,
    video_renderer: Option<Arc<Mutex<dyn VideoFrameRenderer>>>,
    /// Indicates whether the setup was succesfully performed and
    /// we are ready to consume a/v data.
    is_ready: Arc<Once>,
    /// Indicates whether the type of media stream to be played is a live stream.
    stream_type: StreamType,
    /// Decorator used to setup the video sink and process the produced frames.
    render: Arc<Mutex<GStreamerRender>>,
}

impl GStreamerPlayer {
    pub fn new(
        id: usize,
        context_id: &ClientContextId,
        backend_chan: Arc<Mutex<Sender<BackendMsg>>>,
        stream_type: StreamType,
        observer: IpcSender<PlayerEvent>,
        video_renderer: Option<Arc<Mutex<dyn VideoFrameRenderer>>>,
        audio_renderer: Option<Arc<Mutex<dyn AudioRenderer>>>,
        gl_context: Box<dyn PlayerGLContext>,
    ) -> GStreamerPlayer {
        let _ = gst::DebugCategory::new(
            "servoplayer",
            gst::DebugColorFlags::empty(),
            Some("Servo player"),
        );

        Self {
            id,
            context_id: *context_id,
            backend_chan,
            inner: RefCell::new(None),
            observer: Arc::new(Mutex::new(observer)),
            audio_renderer,
            video_renderer,
            is_ready: Arc::new(Once::new()),
            stream_type,
            render: Arc::new(Mutex::new(GStreamerRender::new(gl_context))),
        }
    }

    fn setup(&self) -> Result<(), PlayerError> {
        if self.inner.borrow().is_some() {
            return Ok(());
        }

        // Check that we actually have the elements that we
        // need to make this work.
        for element in vec!["playbin3", "decodebin3", "queue"].iter() {
            if gst::ElementFactory::find(element).is_none() {
                return Err(PlayerError::Backend(format!(
                    "Missing dependency: {}",
                    element
                )));
            }
        }

        let player = gst_play::Play::default();
        let signal_adapter = gst_play::PlaySignalAdapter::new_sync_emit(&player);
        let pipeline = player.pipeline();

        // FIXME(#282): The progressive downloading breaks playback on Windows and Android.
        if !cfg!(any(target_os = "windows", target_os = "android")) {
            // Set player to perform progressive downloading. This will make the
            // player store the downloaded media in a local temporary file for
            // faster playback of already-downloaded chunks.
            let flags = pipeline.property_value("flags");
            let flags_class = match glib::FlagsClass::with_type(flags.type_()) {
                Some(flags) => flags,
                None => {
                    return Err(PlayerError::Backend(
                        "FlagsClass creation failed".to_owned(),
                    ));
                },
            };
            let flags_class = match flags_class.builder_with_value(flags) {
                Some(class) => class,
                None => {
                    return Err(PlayerError::Backend(
                        "FlagsClass creation failed".to_owned(),
                    ));
                },
            };
            let Some(flags) = flags_class.set_by_nick("download").build() else {
                return Err(PlayerError::Backend(
                    "FlagsClass creation failed".to_owned(),
                ));
            };
            pipeline.set_property_from_value("flags", &flags);
        }

        // Set max size for the player buffer.
        pipeline.set_property("buffer-size", MAX_BUFFER_SIZE);

        // Set player position interval update to 0.5 seconds.
        let mut config = player.config();
        config.set_position_update_interval(500u32);
        player
            .set_config(config)
            .map_err(|e| PlayerError::Backend(e.to_string()))?;

        if let Some(ref audio_renderer) = self.audio_renderer {
            let audio_sink = gst::ElementFactory::make("appsink")
                .build()
                .map_err(|error| {
                    PlayerError::Backend(format!("appsink creation failed: {error:?}"))
                })?;

            pipeline.set_property("audio-sink", &audio_sink);

            let audio_sink = audio_sink.dynamic_cast::<gst_app::AppSink>().unwrap();

            let weak_audio_renderer = Arc::downgrade(&audio_renderer);

            audio_sink.set_callbacks(
                gst_app::AppSinkCallbacks::builder()
                    .new_preroll(|_| Ok(gst::FlowSuccess::Ok))
                    .new_sample(move |audio_sink| {
                        let sample = audio_sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                        let buffer = sample.buffer_owned().ok_or(gst::FlowError::Error)?;
                        let audio_info = sample
                            .caps()
                            .and_then(|caps| gst_audio::AudioInfo::from_caps(caps).ok())
                            .ok_or(gst::FlowError::Error)?;
                        let positions = audio_info.positions().ok_or(gst::FlowError::Error)?;

                        let Some(audio_renderer) = weak_audio_renderer.upgrade() else {
                            return Err(gst::FlowError::Flushing);
                        };

                        for position in positions.iter() {
                            let buffer = buffer.clone();
                            let map = match buffer.into_mapped_buffer_readable() {
                                Ok(map) => map,
                                _ => {
                                    return Err(gst::FlowError::Error);
                                },
                            };
                            let chunk = Box::new(GStreamerAudioChunk(map));
                            let channel = position.to_mask() as u32;

                            audio_renderer.lock().unwrap().render(chunk, channel);
                        }
                        Ok(gst::FlowSuccess::Ok)
                    })
                    .build(),
            );
        }

        let video_sink = self.render.lock().unwrap().setup_video_sink(&pipeline)?;

        // There's a known bug in gstreamer that may cause a wrong transition
        // to the ready state while setting the uri property:
        // https://cgit.freedesktop.org/gstreamer/gst-plugins-bad/commit/?id=afbbc3a97ec391c6a582f3c746965fdc3eb3e1f3
        // This may affect things like setting the config, so until the bug is
        // fixed, make sure that state dependent code happens before this line.
        // The estimated version for the fix is 1.14.5 / 1.15.1.
        // https://github.com/servo/servo/issues/22010#issuecomment-432599657
        let uri = match self.stream_type {
            StreamType::Stream => {
                register_servo_media_stream_src().map_err(|error| {
                    PlayerError::Backend(format!(
                        "servomediastreamsrc registration error: {error:?}"
                    ))
                })?;
                "mediastream://".to_value()
            },
            StreamType::Seekable => {
                register_servo_src().map_err(|error| {
                    PlayerError::Backend(format!("servosrc registration error: {error:?}"))
                })?;
                "servosrc://".to_value()
            },
        };
        player.set_property("uri", &uri);

        // No video_renderers no video
        if self.video_renderer.is_none() {
            player.set_video_track_enabled(false);
        }

        let shared_state = Arc::new(Mutex::new(SharedState {
            category: gst::DebugCategory::get("servoplayer").unwrap(),
            play_state: gst_play::PlayState::Stopped,
            pending_input_size: None,
            pending_playback_rate: None,
            metadata: None,
        }));

        *self.inner.borrow_mut() = Some(Arc::new(Mutex::new(PlayerInner {
            player,
            _signal_adapter: signal_adapter.clone(),
            source: None,
            video_sink,
            input_size: Cell::new(0),
            paused: Cell::new(DEFAULT_PAUSED),
            playback_rate: Cell::new(DEFAULT_PLAYBACK_RATE),
            muted: Cell::new(DEFAULT_MUTED),
            volume: Cell::new(DEFAULT_VOLUME),
            stream_type: self.stream_type,
            cat: gst::DebugCategory::get("servoplayer").unwrap(),
            enough_data: Arc::new(AtomicBool::new(false)),
            shared_state: shared_state.clone(),
        })));

        let inner = self.inner.borrow();
        let inner = inner.as_ref().unwrap();

        // Handle `end-of-stream` signal.
        signal_adapter.connect_end_of_stream(glib::clone!(
            #[strong(rename_to = observer)]
            self.observer,
            move |_| {
                let _ = notify!(observer, PlayerEvent::EndOfStream);
            }
        ));

        // Handle `error` signal.
        signal_adapter.connect_error(glib::clone!(
            #[strong(rename_to = observer)]
            self.observer,
            move |_, error, _| {
                let _ = notify!(observer, PlayerEvent::Error(error.to_string()));
            }
        ));

        // Handle `state-changed` signal.
        signal_adapter.connect_state_changed(glib::clone!(
            #[strong(rename_to = observer)]
            self.observer,
            #[strong]
            shared_state,
            move |_, play_state| {
                shared_state.lock().unwrap().play_state = play_state;

                let state = match play_state {
                    gst_play::PlayState::Buffering => Some(PlaybackState::Buffering),
                    gst_play::PlayState::Stopped => Some(PlaybackState::Stopped),
                    gst_play::PlayState::Paused => Some(PlaybackState::Paused),
                    gst_play::PlayState::Playing => Some(PlaybackState::Playing),
                    _ => None,
                };
                if let Some(v) = state {
                    let _ = notify!(observer, PlayerEvent::StateChanged(v));
                }
            }
        ));

        // Handle `position-update` signal.
        signal_adapter.connect_position_updated(glib::clone!(
            #[strong(rename_to = observer)]
            self.observer,
            move |_, position| {
                if let Some(seconds) = position.map(|p| p.seconds_f64()) {
                    let _ = notify!(observer, PlayerEvent::PositionChanged(seconds));
                }
            }
        ));

        // Handle `seek-done` signal.
        signal_adapter.connect_seek_done(glib::clone!(
            #[strong(rename_to = observer)]
            self.observer,
            move |_, position| {
                let _ = notify!(observer, PlayerEvent::SeekDone(position.seconds_f64()));
            }
        ));

        // Handle `media-info-updated` signal.
        signal_adapter.connect_media_info_updated(glib::clone!(
            #[strong(rename_to = observer)]
            self.observer,
            #[strong]
            shared_state,
            move |signal_adapter, info| {
                let Ok(metadata) = metadata_from_media_info(info) else {
                    return;
                };

                let mut shared_state = shared_state.lock().unwrap();

                if shared_state.metadata.as_ref() == Some(&metadata) {
                    return;
                }

                // TODO: Workaround to generate expected `paused` state change event.
                // <https://github.com/servo/servo/issues/40740>
                let mut send_pause_event = false;

                if shared_state.metadata.is_none() && metadata.is_seekable {
                    if shared_state
                        .pending_playback_rate
                        .is_some_and(|playback_rate| playback_rate != DEFAULT_PLAYBACK_RATE)
                    {
                        // The `paused` state change event will be fired after the
                        // seek initiated by the playback rate change has
                        // completed.
                        signal_adapter
                            .play()
                            .set_rate(shared_state.pending_playback_rate.take().unwrap());
                    } else if shared_state.play_state == gst_play::PlayState::Paused {
                        send_pause_event = true;
                    }
                }

                shared_state.metadata = Some(metadata.clone());

                gst::info!(
                    shared_state.category,
                    obj = &signal_adapter.play(),
                    "New metadata: {metadata:?}",
                );

                let _ = notify!(observer, PlayerEvent::MetadataUpdated(metadata));

                if send_pause_event {
                    let _ = notify!(observer, PlayerEvent::StateChanged(PlaybackState::Paused));
                }
            }
        ));

        // Handle `duration-changed` signal.
        signal_adapter.connect_duration_changed(glib::clone!(
            #[strong(rename_to = observer)]
            self.observer,
            #[strong]
            shared_state,
            move |signal_adapter, duration| {
                let duration = duration.map(|duration| {
                    time::Duration::new(
                        duration.seconds(),
                        (duration.nseconds() % 1_000_000_000) as u32,
                    )
                });

                let mut shared_state = shared_state.lock().unwrap();

                let Some(metadata) = shared_state
                    .metadata
                    .as_mut()
                    .filter(|metadata| metadata.duration != duration)
                else {
                    return;
                };

                metadata.duration = duration;

                gst::info!(
                    shared_state.category,
                    obj = &signal_adapter.play(),
                    "New duration: {duration:?}",
                );

                let _ = notify!(observer, PlayerEvent::DurationChanged(duration));
            }
        ));

        if let Some(video_renderer) = self.video_renderer.clone() {
            // Creates a closure that renders a frame using the video_renderer
            // Used in the preroll and sample callbacks
            let render_sample = {
                let render = self.render.clone();
                let observer = self.observer.clone();
                let weak_video_renderer = Arc::downgrade(&video_renderer);

                move |sample: gst::Sample| {
                    let frame = render
                        .lock()
                        .unwrap()
                        .get_frame_from_sample(sample)
                        .map_err(|_| gst::FlowError::Error)?;

                    match weak_video_renderer.upgrade() {
                        Some(video_renderer) => {
                            video_renderer.lock().unwrap().render(frame);
                        },
                        _ => {
                            return Err(gst::FlowError::Flushing);
                        },
                    };

                    let _ = notify!(observer, PlayerEvent::VideoFrameUpdated);
                    Ok(gst::FlowSuccess::Ok)
                }
            };

            // Set video_sink callbacks.
            inner.lock().unwrap().video_sink.set_callbacks(
                gst_app::AppSinkCallbacks::builder()
                    .new_preroll({
                        let render_sample = render_sample.clone();
                        move |video_sink| {
                            render_sample(
                                video_sink.pull_preroll().map_err(|_| gst::FlowError::Eos)?,
                            )
                        }
                    })
                    .new_sample(move |video_sink| {
                        render_sample(video_sink.pull_sample().map_err(|_| gst::FlowError::Eos)?)
                    })
                    .build(),
            );
        };

        let (done_receiver, error_handler_id) = {
            let inner_clone = inner.clone();
            let inner = inner.lock().unwrap();

            let (done_sender, done_receiver) = mpsc::channel();
            let done_sender = Arc::new(Mutex::new(done_sender));

            inner.player.pipeline().connect_closure(
                "source-setup",
                false,
                glib::closure!(
                    #[strong(rename_to = observer)]
                    self.observer,
                    #[strong]
                    done_sender,
                    #[strong]
                    shared_state,
                    #[strong(rename_to = enough_data)]
                    inner.enough_data,
                    #[strong(rename_to = is_ready)]
                    self.is_ready,
                    #[strong(rename_to = stream_type)]
                    self.stream_type,
                    #[weak(rename_to = inner)]
                    inner_clone,
                    move |_pipeline: &gst::Element, source: &gst::Element| {
                        let source = match stream_type {
                            StreamType::Seekable => {
                                let servosrc = source
                                    .clone()
                                    .dynamic_cast::<ServoSrc>()
                                    .expect("Source element is expected to be a ServoSrc!");

                                let mut shared_state = shared_state.lock().unwrap();

                                if shared_state.pending_input_size.is_some_and(|size| size > 0) {
                                    servosrc.set_size(
                                        shared_state.pending_input_size.take().unwrap() as i64,
                                    );
                                }

                                servosrc.set_callbacks(
                                    gst_app::AppSrcCallbacks::builder()
                                        .need_data(glib::clone!(
                                            #[strong]
                                            observer,
                                            #[strong]
                                            done_sender,
                                            #[strong]
                                            enough_data,
                                            #[strong]
                                            is_ready,
                                            move |_, _| {
                                                // We block the caller of the setup method until we get
                                                // the first need-data signal, so we ensure that we
                                                // don't miss any data between the moment the client
                                                // calls setup and the player is actually ready to
                                                // get any data.
                                                is_ready.call_once(|| {
                                                    let _ =
                                                        done_sender.lock().unwrap().send(Ok(()));
                                                });
                                                enough_data.store(false, Ordering::Relaxed);
                                                let _ = notify!(observer, PlayerEvent::NeedData);
                                            }
                                        ))
                                        .enough_data(glib::clone!(
                                            #[strong]
                                            observer,
                                            #[strong]
                                            enough_data,
                                            move |_| {
                                                enough_data.store(true, Ordering::Relaxed);
                                                let _ = notify!(observer, PlayerEvent::EnoughData);
                                            }
                                        ))
                                        .seek_data(glib::clone!(
                                            #[strong]
                                            observer,
                                            #[weak]
                                            servosrc,
                                            #[upgrade_or]
                                            false,
                                            move |_, offset| {
                                                let (ret, ack_channel) = if servosrc
                                                    .set_seek_offset(offset)
                                                {
                                                    let seek_channel =
                                                        Arc::new(Mutex::new(SeekChannel::new()));

                                                    let _ = notify!(
                                                        observer,
                                                        PlayerEvent::SeekData(
                                                            offset,
                                                            seek_channel.lock().unwrap().sender()
                                                        )
                                                    );
                                                    let (ret, ack_channel) =
                                                        seek_channel.lock().unwrap()._await();
                                                    (ret, Some(ack_channel))
                                                } else {
                                                    (true, None)
                                                };

                                                servosrc.set_seek_done();
                                                if let Some(ack_channel) = ack_channel {
                                                    ack_channel.send(()).unwrap();
                                                }
                                                ret
                                            }
                                        ))
                                        .build(),
                                );

                                PlayerSource::Seekable(servosrc)
                            },
                            StreamType::Stream => {
                                let media_stream_src =
                                    source.clone().dynamic_cast::<ServoMediaStreamSrc>().expect(
                                        "Source element is expected to be a ServoMediaStreamSrc!",
                                    );
                                is_ready.call_once(|| {
                                    let _ = notify!(done_sender, Ok(()));
                                });
                                PlayerSource::Stream(media_stream_src)
                            },
                        };

                        inner.lock().unwrap().set_src(source);
                    },
                ),
            );

            let error_handler_id = signal_adapter.connect_error(glib::clone!(
                #[strong]
                done_sender,
                move |signal_adapter, error, _| {
                    let _ = notify!(done_sender, Err(PlayerError::Backend(error.to_string())));
                    signal_adapter.play().stop();
                }
            ));

            let _ = inner.player.pause();

            (done_receiver, error_handler_id)
        };

        let result = done_receiver.recv().unwrap();
        glib::signal::signal_handler_disconnect(&inner.lock().unwrap().player, error_handler_id);
        result
    }
}

macro_rules! inner_player_proxy_getter {
    ($fn_name:ident, $return_type:ty, $default_value:expr_2021) => {
        fn $fn_name(&self) -> $return_type {
            if self.setup().is_err() {
                return $default_value;
            }

            let inner = self.inner.borrow();
            let inner = inner.as_ref().unwrap().lock().unwrap();
            inner.$fn_name()
        }
    };
}

macro_rules! inner_player_proxy {
    ($fn_name:ident, $return_type:ty) => {
        fn $fn_name(&self) -> Result<$return_type, PlayerError> {
            self.setup()?;
            let inner = self.inner.borrow();
            let mut inner = inner.as_ref().unwrap().lock().unwrap();
            inner.$fn_name()
        }
    };

    ($fn_name:ident, $arg1:ident, $arg1_type:ty) => {
        fn $fn_name(&self, $arg1: $arg1_type) -> Result<(), PlayerError> {
            self.setup()?;
            let inner = self.inner.borrow();
            let mut inner = inner.as_ref().unwrap().lock().unwrap();
            inner.$fn_name($arg1)
        }
    };

    ($fn_name:ident, $arg1:ident, $arg1_type:ty, $arg2:ident, $arg2_type:ty) => {
        fn $fn_name(&self, $arg1: $arg1_type, $arg2: $arg2_type) -> Result<(), PlayerError> {
            self.setup()?;
            let inner = self.inner.borrow();
            let mut inner = inner.as_ref().unwrap().lock().unwrap();
            inner.$fn_name($arg1, $arg2)
        }
    };
}

impl Player for GStreamerPlayer {
    inner_player_proxy!(play, ());
    inner_player_proxy!(pause, ());
    inner_player_proxy_getter!(paused, bool, DEFAULT_PAUSED);
    inner_player_proxy!(stop, ());
    inner_player_proxy!(end_of_stream, ());
    inner_player_proxy!(set_input_size, size, u64);
    inner_player_proxy!(set_mute, muted, bool);
    inner_player_proxy_getter!(muted, bool, DEFAULT_MUTED);
    inner_player_proxy!(set_playback_rate, playback_rate, f64);
    inner_player_proxy_getter!(playback_rate, f64, DEFAULT_PLAYBACK_RATE);
    inner_player_proxy!(push_data, data, Vec<u8>);
    inner_player_proxy!(seek, time, f64);
    inner_player_proxy!(set_volume, volume, f64);
    inner_player_proxy_getter!(volume, f64, DEFAULT_VOLUME);
    inner_player_proxy_getter!(buffered, Vec<Range<f64>>, DEFAULT_TIME_RANGES);
    inner_player_proxy_getter!(seekable, Vec<Range<f64>>, DEFAULT_TIME_RANGES);
    inner_player_proxy!(set_stream, stream, &MediaStreamId, only_stream, bool);
    inner_player_proxy!(set_audio_track, stream_index, i32, enabled, bool);
    inner_player_proxy!(set_video_track, stream_index, i32, enabled, bool);

    fn render_use_gl(&self) -> bool {
        self.render.lock().unwrap().is_gl()
    }
}

impl MediaInstance for GStreamerPlayer {
    fn get_id(&self) -> usize {
        self.id
    }

    fn mute(&self, val: bool) -> Result<(), ()> {
        self.set_mute(val).map_err(|_| ())
    }

    fn suspend(&self) -> Result<(), ()> {
        self.pause().map_err(|_| ())
    }

    fn resume(&self) -> Result<(), ()> {
        self.play().map_err(|_| ())
    }
}

impl Drop for GStreamerPlayer {
    fn drop(&mut self) {
        let _ = self.stop();
        let (tx_ack, rx_ack) = mpsc::channel();
        let _ = self
            .backend_chan
            .lock()
            .unwrap()
            .send(BackendMsg::Shutdown {
                context: self.context_id,
                id: self.id,
                tx_ack,
            });
        let _ = rx_ack.recv();
    }
}
