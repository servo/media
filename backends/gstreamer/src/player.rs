use super::BackendError;
use glib;
use glib::*;
use gst;
use gst_app::{self, AppSrcCallbacks};
use gst_player;
use gst_player::{PlayerMediaInfo, PlayerStreamInfoExt};
use ipc_channel::ipc::IpcSender;
use servo_media_player::frame::{Frame, FrameRenderer};
use servo_media_player::metadata::Metadata;
use servo_media_player::{PlaybackState, Player, PlayerEvent, StreamType};
use std::cell::RefCell;
use std::error::Error;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time;
use std::u64;

fn frame_from_sample(sample: &gst::Sample) -> Result<Frame, ()> {
    let caps = sample.get_caps().ok_or_else(|| ())?;
    let s = caps.get_structure(0).ok_or_else(|| ())?;
    let width = s.get("width").ok_or_else(|| ())?;
    let height = s.get("height").ok_or_else(|| ())?;

    let buffer = sample.get_buffer().ok_or_else(|| ())?;
    let map = buffer.map_readable().ok_or_else(|| ())?;
    let data = Vec::from(map.as_slice());

    Ok(Frame::new(width, height, Arc::new(data)))
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

    let seekable = media_info.is_seekable();

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

    Ok(Metadata {
        duration,
        width,
        height,
        format,
        seekable,
        audio_tracks,
        video_tracks,
    })
}

struct PlayerInner {
    player: gst_player::Player,
    appsrc: Option<gst_app::AppSrc>,
    appsink: gst_app::AppSink,
    input_size: u64,
    subscribers: Vec<IpcSender<PlayerEvent>>,
    renderers: Vec<Arc<Mutex<FrameRenderer>>>,
    last_metadata: Option<Metadata>,
}

impl PlayerInner {
    pub fn register_event_handler(&mut self, sender: IpcSender<PlayerEvent>) -> Result<(), BackendError> {
        self.subscribers.push(sender);
        Ok(())
    }

    pub fn register_frame_renderer(&mut self, renderer: Arc<Mutex<FrameRenderer>>) -> Result<(), BackendError> {
        self.renderers.push(renderer);
        Ok(())
    }

    pub fn notify(&self, event: PlayerEvent) {
        for sender in &self.subscribers {
            sender.send(event.clone()).unwrap();
        }
    }

    pub fn render(&self, sample: &gst::Sample) -> Result<(), ()> {
        let frame = frame_from_sample(&sample)?;

        for renderer in &self.renderers {
            renderer.lock().unwrap().render(frame.clone());
        }
        self.notify(PlayerEvent::FrameUpdated);
        Ok(())
    }

    pub fn set_input_size(&mut self, size: u64) -> Result<(), BackendError> {
        // Set input_size to proxy its value, since it
        // could be set by the user before calling .setup().
        self.input_size = size;
        if let Some(ref mut appsrc) = self.appsrc {
            if size > 0 {
                appsrc.set_size(size as i64);
            } else {
                appsrc.set_size(-1); // live source
            }
        }
        Ok(())
    }

    pub fn set_stream_type(&mut self, type_: StreamType) -> Result<(), BackendError> {
        if let Some(ref appsrc) = self.appsrc {
            appsrc.set_stream_type(match type_ {
                StreamType::NonSeekable => gst_app::AppStreamType::Stream,
                StreamType::Seekable => gst_app::AppStreamType::Seekable,
                StreamType::SeekableFast => gst_app::AppStreamType::RandomAccess,
            });
            return Ok(());
        }
        Err(BackendError::PlayerNoAppSrc)
    }

    pub fn play(&mut self) -> Result<(), BackendError> {
        self.player.play();
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), BackendError> {
        self.player.stop();
        self.last_metadata = None;
        self.appsrc = None;
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), BackendError> {
        self.player.pause();
        Ok(())
    }

    pub fn end_of_stream(&mut self) -> Result<(), BackendError> {
        if let Some(ref mut appsrc) = self.appsrc {
            if appsrc.end_of_stream() == gst::FlowReturn::Ok {
                return Ok(());
            }
        }
        Err(BackendError::PlayerEOSFailed)
    }

    pub fn seek(&mut self, time: f64, accurate: bool) -> Result<(), BackendError> {
        if let Some(ref metadata) = self.last_metadata {
            if !metadata.seekable {
                eprintln!("Non seekable stream");
                return Err(BackendError::PlayerNonSeekable);
            }
            if let Some(ref duration) = metadata.duration {
                if duration < &time::Duration::new(time as u64, 0) {
                    eprintln!("Trying to seek out of range");
                    return Err(BackendError::PlayerSeekOutOfRange);
                }
            }
        }

        // XXX Cannot change config while playing
        // Need to create bindings for gst_player_config_set_seek_accurate
        /*let mut config = self.player.get_config();
        config.set_seek_accurate(accurate);
        self.player
            .set_config(config)
            .map_err(|e| BackendError::SetPropertyFailed(e.0))?;*/

        let time = time * 1_000_000_000.;
        self.player.seek(gst::ClockTime::from_nseconds(time as u64));
        Ok(())
    }

    pub fn push_data(&mut self, data: Vec<u8>) -> Result<(), BackendError> {
        if let Some(ref mut appsrc) = self.appsrc {
            let buffer =
                gst::Buffer::from_slice(data).ok_or_else(|| BackendError::PlayerPushDataFailed)?;
            if appsrc.push_buffer(buffer) == gst::FlowReturn::Ok {
                return Ok(());
            }
        }
        Err(BackendError::PlayerPushDataFailed)
    }

    pub fn set_app_src(&mut self, appsrc: gst_app::AppSrc) {
        self.appsrc = Some(appsrc);
    }
}

pub struct GStreamerPlayer {
    inner: RefCell<Option<Arc<Mutex<PlayerInner>>>>,
}

impl GStreamerPlayer {
    pub fn new() -> GStreamerPlayer {
        Self {
            inner: RefCell::new(None),
        }
    }

    fn setup(&self) -> Result<(), BackendError> {
        if self.inner.borrow().is_some() {
            return Ok(());
        }

        let player = gst_player::Player::new(
            /* video renderer */ None, /* signal dispatcher */ None,
        );

        player
            .set_property("uri", &Value::from("appsrc://"))
            .map_err(|e| BackendError::SetPropertyFailed(e.0))?;

        // Set position interval update to 0.5 seconds.
        let mut config = player.get_config();
        config.set_position_update_interval(500u32);
        player
            .set_config(config)
            .map_err(|e| BackendError::SetPropertyFailed(e.0))?;

        let video_sink = gst::ElementFactory::make("appsink", None)
            .ok_or(BackendError::ElementCreationFailed("appsink"))?;
        let pipeline = player.get_pipeline();
        pipeline
            .set_property("video-sink", &video_sink.to_value())
            .map_err(|e| BackendError::SetPropertyFailed(e.0))?;
        let video_sink = video_sink.dynamic_cast::<gst_app::AppSink>().unwrap();
        video_sink.set_caps(&gst::Caps::new_simple(
            "video/x-raw",
            &[
                ("format", &"BGRA"),
                ("pixel-aspect-ratio", &gst::Fraction::from((1, 1))),
            ],
        ));

        *self.inner.borrow_mut() = Some(Arc::new(Mutex::new(PlayerInner {
            player,
            appsrc: None,
            appsink: video_sink,
            input_size: 0,
            subscribers: Vec::new(),
            renderers: Vec::new(),
            last_metadata: None,
        })));

        let inner = self.inner.borrow();
        let inner = inner.as_ref().unwrap();
        let inner_clone = inner.clone();
        inner
            .lock()
            .unwrap()
            .player
            .connect_end_of_stream(move |_| {
                let inner = inner_clone.lock().unwrap();
                inner.notify(PlayerEvent::EndOfStream);
            });

        let inner_clone = inner.clone();
        inner.lock().unwrap().player.connect_error(move |_, _| {
            let inner = inner_clone.lock().unwrap();
            inner.notify(PlayerEvent::Error);
        });

        let inner_clone = inner.clone();
        inner
            .lock()
            .unwrap()
            .player
            .connect_state_changed(move |_, player_state| {
                let state = match player_state {
                    gst_player::PlayerState::Stopped => Some(PlaybackState::Stopped),
                    gst_player::PlayerState::Paused => Some(PlaybackState::Paused),
                    gst_player::PlayerState::Playing => Some(PlaybackState::Playing),
                    _ => None,
                };
                if let Some(v) = state {
                    let inner = inner_clone.lock().unwrap();
                    inner.notify(PlayerEvent::StateChanged(v));
                }
            });

        let inner_clone = inner.clone();
        inner
            .lock()
            .unwrap()
            .player
            .connect_position_updated(move |_, position| {
                if let Some(seconds) = position.seconds() {
                    let inner = inner_clone.lock().unwrap();
                    inner.notify(PlayerEvent::PositionChanged(seconds));
                }
            });

        let inner_clone = inner.clone();
        inner
            .lock()
            .unwrap()
            .player
            .connect_seek_done(move |_, position| {
                if let Some(seconds) = position.seconds() {
                    let inner = inner_clone.lock().unwrap();
                    inner.notify(PlayerEvent::SeekDone(seconds));
                }
            });

        let inner_clone = inner.clone();
        inner
            .lock()
            .unwrap()
            .player
            .connect_media_info_updated(move |_, info| {
                let mut inner = inner_clone.lock().unwrap();
                if let Ok(metadata) = metadata_from_media_info(info) {
                    if inner.last_metadata.as_ref() != Some(&metadata) {
                        inner.last_metadata = Some(metadata.clone());
                        inner.notify(PlayerEvent::MetadataUpdated(metadata));
                    }
                }
            });

        let inner_clone = inner.clone();
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
                    Some(time::Duration::new(seconds.unwrap(), (nanos.unwrap() % 1_000_000_000) as u32))
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
                    inner.notify(PlayerEvent::MetadataUpdated(updated_metadata.unwrap()));
                }
            });

        let inner_clone = inner.clone();
        inner.lock().unwrap().appsink.set_callbacks(
            gst_app::AppSinkCallbacks::new()
                .new_preroll(|_| gst::FlowReturn::Ok)
                .new_sample(move |appsink| {
                    let sample = match appsink.pull_sample() {
                        None => return gst::FlowReturn::Eos,
                        Some(sample) => sample,
                    };

                    match inner_clone.lock().unwrap().render(&sample) {
                        Ok(_) => return gst::FlowReturn::Ok,
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
            let connect_result = pipeline.connect("source-setup", false, move |args| {
                let source = args[1].get::<gst::Element>();
                if source.is_none() {
                    let _ = sender
                        .lock()
                        .unwrap()
                        .send(Err(BackendError::PlayerSourceSetupFailed));
                    return None;
                }
                let source = source.unwrap();
                let mut inner = inner_clone.lock().unwrap();
                let appsrc = source
                    .clone()
                    .dynamic_cast::<gst_app::AppSrc>()
                    .expect("Source element is expected to be an appsrc!");

                appsrc.set_property_format(gst::Format::Bytes);
                if inner.input_size > 0 {
                    appsrc.set_size(inner.input_size as i64);
                }

                let sender_clone = sender.clone();

                let need_data_id = Arc::new(Mutex::new(None));
                let need_data_id_clone = need_data_id.clone();
                *need_data_id.lock().unwrap() = Some(
                    appsrc
                        .connect("need-data", false, move |args| {
                            let _ = sender_clone.lock().unwrap().send(Ok(()));
                            if let Some(id) = need_data_id_clone.lock().unwrap().take() {
                                glib::signal::signal_handler_disconnect(
                                    &args[0].get::<gst::Element>().unwrap(),
                                    id,
                                );
                            }
                            None
                        })
                        .unwrap(),
                );

                let inner_clone = inner_clone.clone();
                appsrc.set_callbacks(
                    AppSrcCallbacks::new()
                        .seek_data(move |_, offset| {
                            inner_clone.lock().unwrap().notify(PlayerEvent::SeekData(offset));
                            true
                        })
                        .build()
                );

                inner.set_app_src(appsrc);

                None
            });

            if connect_result.is_err() {
                let _ = sender_clone
                    .lock()
                    .unwrap()
                    .send(Err(BackendError::PlayerSourceSetupFailed));
            }

            let error_handler_id = inner.player.connect_error(move |player, error| {
                let _ = sender_clone
                    .lock()
                    .unwrap()
                    .send(Err(BackendError::PlayerError(
                        error.description().to_string(),
                    )));
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
    ($fn_name:ident) => (
        fn $fn_name(&self) -> Result<(), BackendError> {
            self.setup()?;
            let inner = self.inner.borrow();
            let mut inner = inner.as_ref().unwrap().lock().unwrap();
            inner.$fn_name()
        }
    );

    ($fn_name:ident, $arg1:ident, $arg1_type:ty) => (
        fn $fn_name(&self, $arg1: $arg1_type) -> Result<(), BackendError> {
            self.setup()?;
            let inner = self.inner.borrow();
            let mut inner = inner.as_ref().unwrap().lock().unwrap();
            inner.$fn_name($arg1)
        }
    )
}
impl Player for GStreamerPlayer {
    type Error = BackendError;

    inner_player_proxy!(register_event_handler, sender, IpcSender<PlayerEvent>);
    inner_player_proxy!(register_frame_renderer, renderer, Arc<Mutex<FrameRenderer>>);
    inner_player_proxy!(play);
    inner_player_proxy!(pause);
    inner_player_proxy!(stop);
    inner_player_proxy!(end_of_stream);
    inner_player_proxy!(set_input_size, size, u64);
    inner_player_proxy!(set_stream_type, type_, StreamType);
    inner_player_proxy!(push_data, data, Vec<u8>);

    fn seek(&self, time: f64, accurate: bool) -> Result<(), BackendError> {
        self.setup()?;
        let inner = self.inner.borrow();
        let mut inner = inner.as_ref().unwrap().lock().unwrap();
        inner.seek(time, accurate)
    }
}

