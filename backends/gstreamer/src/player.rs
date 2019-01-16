use glib;
use glib::*;
use gst::GenericFormattedValue::Percent;
use gst::{self, ElementExtManual};
use gst_app::{self, AppSrcCallbacks, AppStreamType};
use gst_player;
use gst_player::{PlayerMediaInfo, PlayerStreamInfoExt};
use gst_video::{VideoFrame, VideoInfo};
use ipc_channel::ipc::IpcSender;
use servo_media_player::frame::{Frame, FrameRenderer};
use servo_media_player::metadata::Metadata;
use servo_media_player::{PlaybackState, Player, PlayerError, PlayerEvent, StreamType};
use source::{register_servo_src, ServoSrc};
use std::cell::RefCell;
use std::error::Error;
use std::ops::Range;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, Once};
use std::time;
use std::u64;

const MAX_SRC_QUEUE_SIZE: u64 = 50 * 1024 * 1024; // 50 MB.
const MAX_BUFFER_SIZE: i32 = 500 * 1024;

fn frame_from_sample(sample: &gst::Sample) -> Result<Frame, ()> {
    let buffer = sample.get_buffer().ok_or_else(|| ())?;
    let info = sample
        .get_caps()
        .and_then(|caps| VideoInfo::from_caps(caps.as_ref()))
        .ok_or_else(|| ())?;
    let frame = VideoFrame::from_buffer_readable(buffer, &info).or_else(|_| Err(()))?;
    let data = frame.plane_data(0).ok_or_else(|| ())?;

    Ok(Frame::new(
        info.width() as i32,
        info.height() as i32,
        Arc::new(data.to_vec()),
    ))
}

fn metadata_from_media_info(media_info: &PlayerMediaInfo) -> Result<Metadata, ()> {
    let dur = media_info.get_duration();
    let duration = if dur != gst::ClockTime::none() {
        let mut nanos = dur.nanoseconds().ok_or_else(|| ())?;
        nanos = nanos % 1_000_000_000;
        let seconds = dur.seconds().ok_or_else(|| ())?;
        Some(time::Duration::new(seconds, nanos as u32))
    } else {
        None
    };

    let mut audio_tracks = Vec::new();
    let mut video_tracks = Vec::new();

    let format = media_info
        .get_container_format()
        .unwrap_or_else(|| "".to_owned());

    for stream_info in media_info.get_stream_list() {
        let stream_type = stream_info.get_stream_type();
        match stream_type.as_str() {
            "audio" => {
                let codec = stream_info.get_codec().unwrap_or_else(|| "".to_owned());
                audio_tracks.push(codec);
            }
            "video" => {
                let codec = stream_info.get_codec().unwrap_or_else(|| "".to_owned());
                video_tracks.push(codec);
            }
            _ => {}
        }
    }

    let mut width: u32 = 0;
    let height: u32 = if media_info.get_number_of_video_streams() > 0 {
        let first_video_stream = &media_info.get_video_streams()[0];
        width = first_video_stream.get_width() as u32;
        first_video_stream.get_height() as u32
    } else {
        0
    };

    let is_seekable = media_info.is_seekable();
    let is_live = media_info.is_live();

    Ok(Metadata {
        duration,
        width,
        height,
        format,
        is_seekable,
        audio_tracks,
        video_tracks,
        is_live,
    })
}

struct PlayerInner {
    player: gst_player::Player,
    servosrc: Option<ServoSrc>,
    appsink: gst_app::AppSink,
    input_size: u64,
    rate: f64,
    stream_type: Option<AppStreamType>,
    last_metadata: Option<Metadata>,
}

impl PlayerInner {
    pub fn set_input_size(&mut self, size: u64) -> Result<(), PlayerError> {
        // Set input_size to proxy its value, since it
        // could be set by the user before calling .setup().
        self.input_size = size;
        if let Some(ref mut servosrc) = self.servosrc {
            if size > 0 {
                servosrc.set_size(size as i64);
            } else {
                servosrc.set_size(-1); // live source
            }
        }
        Ok(())
    }

    pub fn set_rate(&mut self, rate: f64) -> Result<(), PlayerError> {
        // This method may be called before the player setup is done, so we safe the rate value
        // and set it once the player is ready and after getting the media info
        self.rate = rate;
        if let Some(ref metadata) = self.last_metadata {
            if !metadata.is_seekable {
                eprintln!("Player must be seekable in order to set the playback rate");
                return Err(PlayerError::NonSeekableStream);
            }
            self.player.set_rate(rate);
        }
        Ok(())
    }

    pub fn set_stream_type(&mut self, type_: StreamType) -> Result<(), PlayerError> {
        let type_ = match type_ {
            StreamType::Stream => AppStreamType::Stream,
            StreamType::Seekable => AppStreamType::Seekable,
            StreamType::RandomAccess => AppStreamType::RandomAccess,
        };
        // Set stream_type to proxy its value, since it
        // could be set by the user before calling .setup().
        self.stream_type = Some(type_);
        if let Some(ref servosrc) = self.servosrc {
            servosrc.set_stream_type(type_);
        }
        Ok(())
    }

    pub fn play(&mut self) -> Result<(), PlayerError> {
        self.player.play();
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), PlayerError> {
        self.player.stop();
        self.last_metadata = None;
        self.servosrc = None;
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), PlayerError> {
        self.player.pause();
        Ok(())
    }

    pub fn end_of_stream(&mut self) -> Result<(), PlayerError> {
        if let Some(ref mut servosrc) = self.servosrc {
            if servosrc.end_of_stream() == gst::FlowReturn::Ok {
                return Ok(());
            }
        }
        Err(PlayerError::EOSFailed)
    }

    pub fn seek(&mut self, time: f64) -> Result<(), PlayerError> {
        // XXX Support AppStreamType::RandomAccess. The callback model changes
        // if the stream type is set to RandomAccess (i.e. the seek-data
        // callback is received right after pushing the first chunk of data,
        // even if player.seek() is not called).
        if self.stream_type.is_none() || self.stream_type.unwrap() != AppStreamType::Seekable {
            return Err(PlayerError::NonSeekableStream);
        }
        if let Some(ref metadata) = self.last_metadata {
            if let Some(ref duration) = metadata.duration {
                if duration < &time::Duration::new(time as u64, 0) {
                    eprintln!("Trying to seek out of range");
                    return Err(PlayerError::SeekOutOfRange);
                }
            }
        }

        let time = time * 1_000_000_000.;
        self.player.seek(gst::ClockTime::from_nseconds(time as u64));
        Ok(())
    }

    pub fn set_volume(&mut self, value: f64) -> Result<(), PlayerError> {
        self.player.set_volume(value);
        Ok(())
    }

    pub fn push_data(&mut self, data: Vec<u8>) -> Result<(), PlayerError> {
        if let Some(ref mut servosrc) = self.servosrc {
            if servosrc.get_current_level_bytes() + data.len() as u64 > servosrc.get_max_bytes() {
                return Err(PlayerError::EnoughData);
            }
            let buffer =
                gst::Buffer::from_slice(data).ok_or_else(|| PlayerError::BufferPushFailed)?;
            if servosrc.push_buffer(buffer) == gst::FlowReturn::Ok {
                return Ok(());
            }
        }
        Err(PlayerError::BufferPushFailed)
    }

    pub fn set_src(&mut self, servosrc: ServoSrc) {
        self.servosrc = Some(servosrc);
    }

    pub fn buffered(&mut self) -> Result<Vec<Range<u32>>, PlayerError> {
        let mut result = vec![];
        let pipeline = self.player.get_pipeline();
        let mut buffering = gst::Query::new_buffering(gst::Format::Percent);
        if pipeline.query(&mut buffering) {
            let ranges = buffering.get_ranges();
            for i in 0..ranges.len() {
                let start = ranges[i].0;
                let end = ranges[i].1;
                let start = if let Percent(start) = start {
                    start.unwrap()
                } else {
                    0
                } / (gst::FORMAT_PERCENT_MAX / 100);
                let end = if let Percent(end) = end {
                    end.unwrap()
                } else {
                    0
                } / (gst::FORMAT_PERCENT_MAX / 100);
                result.push(Range { start, end });
            }
        }

        Ok(result)
    }
}

type PlayerEventObserver = IpcSender<PlayerEvent>;
struct PlayerEventObserverList {
    observers: Vec<PlayerEventObserver>,
}

impl PlayerEventObserverList {
    fn new() -> Self {
        Self {
            observers: Vec::new(),
        }
    }

    fn register(&mut self, observer: PlayerEventObserver) {
        self.observers.push(observer);
    }

    fn notify(&self, event: PlayerEvent) {
        for observer in &self.observers {
            observer.send(event.clone()).unwrap();
        }
    }
}

struct FrameRendererList {
    renderers: Vec<Arc<Mutex<FrameRenderer>>>,
}

impl FrameRendererList {
    fn new() -> Self {
        Self {
            renderers: Vec::new(),
        }
    }

    fn register(&mut self, renderer: Arc<Mutex<FrameRenderer>>) {
        self.renderers.push(renderer);
    }

    fn render(&self, sample: &gst::Sample) -> Result<(), ()> {
        let frame = frame_from_sample(&sample)?;

        for renderer in &self.renderers {
            renderer.lock().unwrap().render(frame.clone());
        }
        Ok(())
    }
}

pub struct GStreamerPlayer {
    inner: RefCell<Option<Arc<Mutex<PlayerInner>>>>,
    observers: Arc<Mutex<PlayerEventObserverList>>,
    renderers: Arc<Mutex<FrameRendererList>>,
    /// Indicates whether the setup was succesfully performed and
    /// we are ready to consume a/v data.
    is_ready: Arc<Once>,
}

impl GStreamerPlayer {
    pub fn new() -> GStreamerPlayer {
        Self {
            inner: RefCell::new(None),
            observers: Arc::new(Mutex::new(PlayerEventObserverList::new())),
            renderers: Arc::new(Mutex::new(FrameRendererList::new())),
            is_ready: Arc::new(Once::new()),
        }
    }

    fn setup(&self) -> Result<(), PlayerError> {
        if self.inner.borrow().is_some() {
            return Ok(());
        }

        // Check that we actually have the elements that we
        // need to make this work.
        for element in vec!["playbin", "queue"].iter() {
            if gst::ElementFactory::find(element).is_none() {
                return Err(PlayerError::Backend(format!(
                    "Missing dependency: {}",
                    element
                )));
            }
        }

        if !register_servo_src() {
            return Err(PlayerError::Backend(
                "servosrc registration error".to_owned(),
            ));
        }

        let player = gst_player::Player::new(
            /* video renderer */ None, /* signal dispatcher */ None,
        );

        let pipeline = player.get_pipeline();

        // Set player to perform progressive downloading. This will make the
        // player store the downloaded media in a local temporary file for
        // faster playback of already-downloaded chunks.
        let flags = pipeline
            .get_property("flags")
            .map_err(|e| PlayerError::Backend(e.0.to_owned()))?;
        let flags_class = match FlagsClass::new(flags.type_()) {
            Some(flags) => flags,
            None => {
                return Err(PlayerError::Backend(
                    "FlagsClass creation failed".to_owned(),
                ))
            }
        };
        let flags_class = match flags_class.builder_with_value(flags) {
            Some(class) => class,
            None => {
                return Err(PlayerError::Backend(
                    "FlagsClass creation failed".to_owned(),
                ))
            }
        };
        let flags = match flags_class.set_by_nick("download").build() {
            Some(flags) => flags,
            None => {
                return Err(PlayerError::Backend(
                    "FlagsClass creation failed".to_owned(),
                ))
            }
        };
        pipeline
            .set_property("flags", &flags)
            .map_err(|e| PlayerError::Backend(e.0.to_owned()))?;

        // Set max size for the player buffer.
        pipeline
            .set_property("buffer-size", &MAX_BUFFER_SIZE)
            .map_err(|e| PlayerError::Backend(e.0.to_owned()))?;

        // Set player position interval update to 0.5 seconds.
        let mut config = player.get_config();
        config.set_position_update_interval(500u32);
        player
            .set_config(config)
            .map_err(|e| PlayerError::Backend(e.to_string()))?;

        let video_sink = gst::ElementFactory::make("appsink", None)
            .ok_or(PlayerError::Backend("appsink creation failed".to_owned()))?;
        pipeline
            .set_property("video-sink", &video_sink.to_value())
            .map_err(|e| PlayerError::Backend(e.to_string()))?;
        let video_sink = video_sink.dynamic_cast::<gst_app::AppSink>().unwrap();
        video_sink.set_caps(&gst::Caps::new_simple(
            "video/x-raw",
            &[
                ("format", &"BGRA"),
                ("pixel-aspect-ratio", &gst::Fraction::from((1, 1))),
            ],
        ));

        // There's a known bug in gstreamer that may cause a wrong transition
        // to the ready state while setting the uri property:
        // http://cgit.freedesktop.org/gstreamer/gst-plugins-bad/commit/?id=afbbc3a97ec391c6a582f3c746965fdc3eb3e1f3
        // This may affect things like setting the config, so until the bug is
        // fixed, make sure that state dependent code happens before this line.
        // The estimated version for the fix is 1.14.5 / 1.15.1.
        // https://github.com/servo/servo/issues/22010#issuecomment-432599657
        player
            .set_property("uri", &Value::from("servosrc://"))
            .map_err(|e| PlayerError::Backend(e.to_string()))?;

        *self.inner.borrow_mut() = Some(Arc::new(Mutex::new(PlayerInner {
            player,
            servosrc: None,
            appsink: video_sink,
            input_size: 0,
            rate: 1.0,
            stream_type: None,
            last_metadata: None,
        })));

        let inner = self.inner.borrow();
        let inner = inner.as_ref().unwrap();
        let observers = self.observers.clone();
        // Handle `end-of-stream` signal.
        inner
            .lock()
            .unwrap()
            .player
            .connect_end_of_stream(move |_| {
                observers.lock().unwrap().notify(PlayerEvent::EndOfStream);
            });

        let observers = self.observers.clone();
        // Handle `error` signal
        inner.lock().unwrap().player.connect_error(move |_, _| {
            observers.lock().unwrap().notify(PlayerEvent::Error);
        });

        let observers = self.observers.clone();
        // Handle `state-changed` signal.
        inner
            .lock()
            .unwrap()
            .player
            .connect_state_changed(move |_, player_state| {
                let state = match player_state {
                    gst_player::PlayerState::Buffering => Some(PlaybackState::Buffering),
                    gst_player::PlayerState::Stopped => Some(PlaybackState::Stopped),
                    gst_player::PlayerState::Paused => Some(PlaybackState::Paused),
                    gst_player::PlayerState::Playing => Some(PlaybackState::Playing),
                    _ => None,
                };
                if let Some(v) = state {
                    observers
                        .lock()
                        .unwrap()
                        .notify(PlayerEvent::StateChanged(v));
                }
            });

        let observers = self.observers.clone();
        // Handle `position-update` signal.
        inner
            .lock()
            .unwrap()
            .player
            .connect_position_updated(move |_, position| {
                if let Some(seconds) = position.seconds() {
                    observers
                        .lock()
                        .unwrap()
                        .notify(PlayerEvent::PositionChanged(seconds));
                }
            });

        let observers = self.observers.clone();
        // Handle `seek-done` signal.
        inner
            .lock()
            .unwrap()
            .player
            .connect_seek_done(move |_, position| {
                if let Some(seconds) = position.seconds() {
                    observers
                        .lock()
                        .unwrap()
                        .notify(PlayerEvent::SeekDone(seconds));
                }
            });

        // Handle `media-info-updated` signal.
        let inner_clone = inner.clone();
        let observers = self.observers.clone();
        inner
            .lock()
            .unwrap()
            .player
            .connect_media_info_updated(move |_, info| {
                let mut inner = inner_clone.lock().unwrap();
                if let Ok(metadata) = metadata_from_media_info(info) {
                    if inner.last_metadata.as_ref() != Some(&metadata) {
                        inner.last_metadata = Some(metadata.clone());
                        if metadata.is_seekable {
                            inner.player.set_rate(inner.rate);
                        }
                        observers
                            .lock()
                            .unwrap()
                            .notify(PlayerEvent::MetadataUpdated(metadata));
                    }
                }
            });

        // Handle `duration-changed` signal.
        let inner_clone = inner.clone();
        let observers = self.observers.clone();
        inner
            .lock()
            .unwrap()
            .player
            .connect_duration_changed(move |_, duration| {
                let duration = if duration != gst::ClockTime::none() {
                    let nanos = duration.nanoseconds();
                    if nanos.is_none() {
                        eprintln!("Could not get duration nanoseconds");
                        return;
                    }
                    let seconds = duration.seconds();
                    if seconds.is_none() {
                        eprintln!("Could not get duration seconds");
                        return;
                    }
                    Some(time::Duration::new(
                        seconds.unwrap(),
                        (nanos.unwrap() % 1_000_000_000) as u32,
                    ))
                } else {
                    None
                };
                let mut inner = inner_clone.lock().unwrap();
                let mut updated_metadata = None;
                if let Some(ref mut metadata) = inner.last_metadata {
                    metadata.duration = duration;
                    updated_metadata = Some(metadata.clone());
                }
                if updated_metadata.is_some() {
                    observers
                        .lock()
                        .unwrap()
                        .notify(PlayerEvent::MetadataUpdated(updated_metadata.unwrap()));
                }
            });

        let observers = self.observers.clone();
        let renderers = self.renderers.clone();
        // Set appsink callbacks.
        inner.lock().unwrap().appsink.set_callbacks(
            gst_app::AppSinkCallbacks::new()
                .new_preroll(|_| gst::FlowReturn::Ok)
                .new_sample(move |appsink| {
                    let sample = match appsink.pull_sample() {
                        None => return gst::FlowReturn::Eos,
                        Some(sample) => sample,
                    };

                    match renderers.lock().unwrap().render(&sample) {
                        Ok(_) => {
                            observers.lock().unwrap().notify(PlayerEvent::FrameUpdated);
                            return gst::FlowReturn::Ok;
                        }
                        Err(_) => return gst::FlowReturn::Error,
                    };
                })
                .build(),
        );

        let (receiver, error_handler_id) = {
            let inner_clone = inner.clone();
            let mut inner = inner.lock().unwrap();
            let pipeline = inner.player.get_pipeline();

            let (sender, receiver) = mpsc::channel();

            let sender = Arc::new(Mutex::new(sender));
            let sender_clone = sender.clone();
            let is_ready_clone = self.is_ready.clone();
            let observers = self.observers.clone();
            let connect_result = pipeline.connect("source-setup", false, move |args| {
                let source = match args[1].get::<gst::Element>() {
                    Some(source) => source,
                    None => {
                        let _ = sender
                            .lock()
                            .unwrap()
                            .send(Err(PlayerError::Backend("Source setup failed".to_owned())));
                        return None;
                    }
                };

                let mut inner = inner_clone.lock().unwrap();
                let servosrc = source
                    .clone()
                    .dynamic_cast::<ServoSrc>()
                    .expect("Source element is expected to be a servosrc!");

                servosrc.set_max_bytes(MAX_SRC_QUEUE_SIZE);
                servosrc.set_property_block(false);

                servosrc.set_property_format(gst::Format::Bytes);
                if inner.input_size > 0 {
                    servosrc.set_size(inner.input_size as i64);
                }

                if let Some(ref stream_type) = inner.stream_type {
                    servosrc.set_stream_type(*stream_type);
                }

                let sender_clone = sender.clone();
                let is_ready_ = is_ready_clone.clone();
                let observers_ = observers.clone();
                let observers__ = observers.clone();
                let observers___ = observers.clone();
                servosrc.set_callbacks(
                    AppSrcCallbacks::new()
                        .need_data(move |_, _| {
                            // We block the caller of the setup method until we get
                            // the first need-data signal, so we ensure that we
                            // don't miss any data between the moment the client
                            // calls setup and the player is actually ready to
                            // get any data.
                            is_ready_.call_once(|| {
                                let _ = sender_clone.lock().unwrap().send(Ok(()));
                            });
                            observers_.lock().unwrap().notify(PlayerEvent::NeedData);
                        })
                        .enough_data(move |_| {
                            observers__.lock().unwrap().notify(PlayerEvent::EnoughData);
                        })
                        .seek_data(move |_, offset| {
                            observers___
                                .lock()
                                .unwrap()
                                .notify(PlayerEvent::SeekData(offset));
                            true
                        })
                        .build(),
                );

                inner.set_src(servosrc);

                None
            });

            if connect_result.is_err() {
                let _ = sender_clone
                    .lock()
                    .unwrap()
                    .send(Err(PlayerError::Backend("Source setup failed".to_owned())));
            }

            let error_handler_id = inner.player.connect_error(move |player, error| {
                let _ = sender_clone
                    .lock()
                    .unwrap()
                    .send(Err(PlayerError::Backend(error.description().to_string())));
                player.stop();
            });

            let _ = inner.pause();

            (receiver, error_handler_id)
        };

        let result = receiver.recv().unwrap();
        glib::signal::signal_handler_disconnect(&inner.lock().unwrap().player, error_handler_id);
        result
    }
}

macro_rules! inner_player_proxy {
    ($fn_name:ident, $return_type:ty) => (
        fn $fn_name(&self) -> Result<$return_type, PlayerError> {
            self.setup()?;
            let inner = self.inner.borrow();
            let mut inner = inner.as_ref().unwrap().lock().unwrap();
            inner.$fn_name()
        }
    );

    ($fn_name:ident, $arg1:ident, $arg1_type:ty) => (
        fn $fn_name(&self, $arg1: $arg1_type) -> Result<(), PlayerError> {
            self.setup()?;
            let inner = self.inner.borrow();
            let mut inner = inner.as_ref().unwrap().lock().unwrap();
            inner.$fn_name($arg1)
        }
    )
}

impl Player for GStreamerPlayer {
    inner_player_proxy!(play, ());
    inner_player_proxy!(pause, ());
    inner_player_proxy!(stop, ());
    inner_player_proxy!(end_of_stream, ());
    inner_player_proxy!(set_input_size, size, u64);
    inner_player_proxy!(set_rate, rate, f64);
    inner_player_proxy!(set_stream_type, type_, StreamType);
    inner_player_proxy!(push_data, data, Vec<u8>);
    inner_player_proxy!(seek, time, f64);
    inner_player_proxy!(set_volume, value, f64);
    inner_player_proxy!(buffered, Vec<Range<u32>>);

    fn register_event_handler(&self, sender: IpcSender<PlayerEvent>) {
        self.observers.lock().unwrap().register(sender);
    }

    fn register_frame_renderer(&self, renderer: Arc<Mutex<FrameRenderer>>) {
        self.renderers.lock().unwrap().register(renderer);
    }
}
