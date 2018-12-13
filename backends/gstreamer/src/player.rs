use super::BackendError;
use glib;
use glib::*;
use gst;
use gst_app::{self, AppSrcCallbacks, AppStreamType};
use gst_player;
use gst_player::{PlayerMediaInfo, PlayerStreamInfoExt};
use gst_video::{VideoFrame, VideoInfo};
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

    Ok(Metadata {
        duration,
        width,
        height,
        format,
        is_seekable,
        audio_tracks,
        video_tracks,
    })
}

struct PlayerInner {
    player: gst_player::Player,
    appsrc: Option<gst_app::AppSrc>,
    appsink: gst_app::AppSink,
    input_size: u64,
    rate: f64,
    stream_type: Option<AppStreamType>,
    subscribers: Vec<IpcSender<PlayerEvent>>,
    renderers: Vec<Arc<Mutex<FrameRenderer>>>,
    last_metadata: Option<Metadata>,
}

impl PlayerInner {
    pub fn register_event_handler(
        &mut self,
        sender: IpcSender<PlayerEvent>,
    ) -> Result<(), BackendError> {
        self.subscribers.push(sender);
        Ok(())
    }

    pub fn register_frame_renderer(
        &mut self,
        renderer: Arc<Mutex<FrameRenderer>>,
    ) -> Result<(), BackendError> {
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

    pub fn set_rate(&mut self, rate: f64) -> Result<(), BackendError> {
        // This method may be called before the player setup is done, so we safe the rate value
        // and set it once the player is ready and after getting the media info
        self.rate = rate;
        if let Some(ref metadata) = self.last_metadata {
            if !metadata.is_seekable {
                eprintln!("Player must be seekable in order to set the playback rate");
                return Err(BackendError::PlayerNonSeekable);
            }
            self.player.set_rate(rate);
        }
        Ok(())
    }

    pub fn set_stream_type(&mut self, type_: StreamType) -> Result<(), BackendError> {
        let type_ = match type_ {
            StreamType::Stream => AppStreamType::Stream,
            StreamType::Seekable => AppStreamType::Seekable,
            StreamType::RandomAccess => AppStreamType::RandomAccess,
        };
        // Set stream_type to proxy its value, since it
        // could be set by the user before calling .setup().
        self.stream_type = Some(type_);
        if let Some(ref appsrc) = self.appsrc {
            appsrc.set_stream_type(type_);
        }
        Ok(())
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

    pub fn seek(&mut self, time: f64) -> Result<(), BackendError> {
        // XXX Support AppStreamType::RandomAccess. The callback model changes
        // if the stream type is set to RandomAccess (i.e. the seek-data
        // callback is received right after pushing the first chunk of data,
        // even if player.seek() is not called).
        if self.stream_type.is_none() || self.stream_type.unwrap() != AppStreamType::Seekable {
            return Err(BackendError::PlayerNonSeekable);
        }
        if let Some(ref metadata) = self.last_metadata {
            if let Some(ref duration) = metadata.duration {
                if duration < &time::Duration::new(time as u64, 0) {
                    eprintln!("Trying to seek out of range");
                    return Err(BackendError::PlayerSeekOutOfRange);
                }
            }
        }

        let time = time * 1_000_000_000.;
        self.player.seek(gst::ClockTime::from_nseconds(time as u64));
        Ok(())
    }

    pub fn set_volume(&mut self, value: f64) -> Result<(), BackendError> {
        self.player.set_volume(value);
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

        // Check that we actually have the elements that we
        // need to make this work.
        for element in vec!["playbin", "queue"].iter() {
            if gst::ElementFactory::find(element).is_none() {
                return Err(BackendError::MissingElement(element));
            }
        }

        let player = gst_player::Player::new(
            /* video renderer */ None, /* signal dispatcher */ None,
        );

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

        // There's a known bug in gstreamer that may cause a wrong transition
        // to the ready state while setting the uri property:
        // http://cgit.freedesktop.org/gstreamer/gst-plugins-bad/commit/?id=afbbc3a97ec391c6a582f3c746965fdc3eb3e1f3
        // This may affect things like setting the config, so until the bug is
        // fixed, make sure that state dependent code happens before this line.
        // The estimated version for the fix is 1.14.5 / 1.15.1.
        // https://github.com/servo/servo/issues/22010#issuecomment-432599657
        player
            .set_property("uri", &Value::from("appsrc://"))
            .map_err(|e| BackendError::SetPropertyFailed(e.0))?;

        *self.inner.borrow_mut() = Some(Arc::new(Mutex::new(PlayerInner {
            player,
            appsrc: None,
            appsink: video_sink,
            input_size: 0,
            rate: 1.0,
            stream_type: None,
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
                        if metadata.is_seekable {
                            inner.player.set_rate(inner.rate);
                        }
                        inner.notify(PlayerEvent::MetadataUpdated(metadata));
                    }
                }
            });

        let inner_clone = inner.clone();
        inner
            .lock()
            .unwrap()
            .player
            .connect_property_rate_notify(move |_| {
                let inner = inner_clone.lock().unwrap();
                inner.notify(PlayerEvent::RateChanged(inner.rate));
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

                if let Some(ref stream_type) = inner.stream_type {
                    appsrc.set_stream_type(*stream_type);
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
                            inner_clone
                                .lock()
                                .unwrap()
                                .notify(PlayerEvent::SeekData(offset));
                            true
                        })
                        .build(),
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
    inner_player_proxy!(set_rate, rate, f64);
    inner_player_proxy!(set_stream_type, type_, StreamType);
    inner_player_proxy!(push_data, data, Vec<u8>);
    inner_player_proxy!(seek, time, f64);
    inner_player_proxy!(set_volume, value, f64);
}
