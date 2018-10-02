use glib;
use glib::*;
use gst;
use gst_app;
use gst_player;
use gst_player::{PlayerMediaInfo, PlayerStreamInfoExt};
use ipc_channel::ipc::IpcSender;
use servo_media_player::frame::{Frame, FrameRenderer};
use servo_media_player::metadata::Metadata;
use servo_media_player::{PlaybackState, Player, PlayerEvent};
use std::cell::RefCell;
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
    pub fn register_event_handler(&mut self, sender: IpcSender<PlayerEvent>) {
        self.subscribers.push(sender);
    }

    pub fn register_frame_renderer(&mut self, renderer: Arc<Mutex<FrameRenderer>>) {
        self.renderers.push(renderer);
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

    pub fn set_input_size(&mut self, size: u64) {
        self.input_size = size;
    }

    pub fn play(&mut self) {
        self.player.play();
    }

    pub fn stop(&mut self) {
        self.player.stop();
        self.last_metadata = None;
        self.appsrc = None;
    }

    pub fn pause(&mut self) {
        self.player.pause();
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

    fn setup(&self) -> Result<(), ()> {
        if self.inner.borrow().is_some() {
            return Ok(());
        }
        let player = gst_player::Player::new(
            /* video renderer */ None, /* signal dispatcher */ None,
        );
        player
            .set_property("uri", &Value::from("appsrc://"))
            .or_else(|_| Err(()))?;

        // Disable periodic position updates for now.
        let mut config = player.get_config();
        config.set_position_update_interval(0u32);
        player.set_config(config).or_else(|_| Err(()))?;

        let video_sink = gst::ElementFactory::make("appsink", None).ok_or_else(|| ())?;
        let pipeline = player.get_pipeline();
        pipeline
            .set_property("video-sink", &video_sink.to_value())
            .or_else(|_| Err(()))?;
        let video_sink = video_sink
            .dynamic_cast::<gst_app::AppSink>()
            .or_else(|_| Err(()))?;
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
                let inner = &inner_clone;
                let guard = inner.lock().unwrap();

                guard.notify(PlayerEvent::EndOfStream);
            });

        let inner_clone = inner.clone();
        inner.lock().unwrap().player.connect_error(move |_, _| {
            let inner = &inner_clone;
            let guard = inner.lock().unwrap();

            guard.notify(PlayerEvent::Error);
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
                    let inner = &inner_clone;
                    let guard = inner.lock().unwrap();

                    guard.notify(PlayerEvent::StateChanged(v));
                }
            });

        let inner_clone = inner.clone();
        inner
            .lock()
            .unwrap()
            .player
            .connect_media_info_updated(move |_, info| {
                let inner = &inner_clone;
                let mut guard = inner.lock().unwrap();

                if let Ok(metadata) = metadata_from_media_info(info) {
                    if guard.last_metadata.as_ref() != Some(&metadata) {
                        guard.last_metadata = Some(metadata.clone());
                        guard.notify(PlayerEvent::MetadataUpdated(metadata));
                    }
                }
            });

        inner
            .lock()
            .unwrap()
            .player
            .connect_duration_changed(move |_, duration| {
                let mut seconds = duration / 1_000_000_000;
                let mut minutes = seconds / 60;
                seconds %= 60;
                minutes %= 60;
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

        let inner_clone = inner.clone();
        let (receiver, error_id) = {
            let mut inner = inner.lock().unwrap();
            let pipeline = inner.player.get_pipeline();

            let (sender, receiver) = mpsc::channel();

            let sender = Arc::new(Mutex::new(sender));
            let sender_clone = sender.clone();
            pipeline
                .connect("source-setup", false, move |args| {
                    let mut inner = inner_clone.lock().unwrap();

                    if let Some(source) = args[1].get::<gst::Element>() {
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

                        inner.set_app_src(appsrc);
                    } else {
                        let _ = sender.lock().unwrap().send(Err(()));
                    }

                    None
                })
                .unwrap();

            let error_id = inner.player.connect_error(move |_, _| {
                let _ = sender_clone.lock().unwrap().send(Err(()));
            });

            inner.pause();

            (receiver, error_id)
        };

        glib::signal::signal_handler_disconnect(&inner.lock().unwrap().player, error_id);

        match receiver.recv().unwrap() {
            Ok(_) => return Ok(()),
            Err(_) => return Err(()),
        };
    }
}

impl Player for GStreamerPlayer {
    fn register_event_handler(&self, sender: IpcSender<PlayerEvent>) {
        let _ = self.setup();
        let inner = self.inner.borrow();
        inner
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .register_event_handler(sender);
    }

    fn register_frame_renderer(&self, renderer: Arc<Mutex<FrameRenderer>>) {
        let _ = self.setup();
        let inner = self.inner.borrow();
        inner
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .register_frame_renderer(renderer);
    }

    fn set_input_size(&self, size: u64) {
        let _ = self.setup();
        // Keep inner's .set_input_size() to proxy its value, since it
        // could be set by the user before calling .setup()
        let inner = self.inner.borrow();
        let mut inner = inner.as_ref().unwrap().lock().unwrap();
        inner.set_input_size(size);
        if let Some(ref mut appsrc) = inner.appsrc {
            if size > 0 {
                appsrc.set_size(size as i64);
            } else {
                appsrc.set_size(-1); // live source
            }
        }
    }

    fn play(&self) {
        let _ = self.setup();
        let inner = self.inner.borrow();
        inner.as_ref().unwrap().lock().unwrap().play();
    }

    fn pause(&self) {
        let _ = self.setup();
        let inner = self.inner.borrow();
        inner.as_ref().unwrap().lock().unwrap().pause();
    }

    fn stop(&self) {
        let _ = self.setup();
        let inner = self.inner.borrow();
        inner.as_ref().unwrap().lock().unwrap().stop();
    }

    fn push_data(&self, data: Vec<u8>) -> Result<(), ()> {
        let _ = self.setup();
        let inner = self.inner.borrow();
        let mut inner = inner.as_ref().unwrap().lock().unwrap();
        if let Some(ref mut appsrc) = inner.appsrc {
            let buffer = gst::Buffer::from_slice(data).ok_or_else(|| ())?;
            if appsrc.push_buffer(buffer) == gst::FlowReturn::Ok {
                return Ok(());
            }
        }
        Err(())
    }

    fn end_of_stream(&self) -> Result<(), ()> {
        let _ = self.setup();
        let inner = self.inner.borrow();
        let mut inner = inner.as_ref().unwrap().lock().unwrap();
        if let Some(ref mut appsrc) = inner.appsrc {
            if appsrc.end_of_stream() == gst::FlowReturn::Ok {
                return Ok(());
            }
        }
        Err(())
    }
}
