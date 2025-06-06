use crate::media_stream::GstreamerMediaSocket;
use byte_slice_cast::*;
use gst;
use gst::prelude::*;
use gst_app::{AppSrc, AppSrcCallbacks};
use gst_audio;
use servo_media_audio::block::{Chunk, FRAMES_PER_BLOCK};
use servo_media_audio::render_thread::AudioRenderThreadMsg;
use servo_media_audio::sink::{AudioSink, AudioSinkError};
use servo_media_streams::MediaSocket;
use std::cell::{Cell, RefCell};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::Builder;

const DEFAULT_SAMPLE_RATE: f32 = 44100.;

pub struct GStreamerAudioSink {
    pipeline: gst::Pipeline,
    appsrc: Arc<AppSrc>,
    sample_rate: Cell<f32>,
    audio_info: RefCell<Option<gst_audio::AudioInfo>>,
    sample_offset: Cell<u64>,
}

impl GStreamerAudioSink {
    pub fn new() -> Result<Self, AudioSinkError> {
        if let Some(category) = gst::DebugCategory::get("openslessink") {
            category.set_threshold(gst::DebugLevel::Trace);
        }
        gst::init().map_err(|error| AudioSinkError::Backend(format!("GStreamer init failed: {error:?}")))?;

        let appsrc = gst::ElementFactory::make("appsrc")
            .build()
            .map_err(|error| AudioSinkError::Backend(format!("appsrc creation failed: {error:?}")))?;
        let appsrc = appsrc.downcast::<AppSrc>().unwrap();

        Ok(Self {
            pipeline: gst::Pipeline::new(),
            appsrc: Arc::new(appsrc),
            sample_rate: Cell::new(DEFAULT_SAMPLE_RATE),
            audio_info: RefCell::new(None),
            sample_offset: Cell::new(0),
        })
    }
}

impl GStreamerAudioSink {
    fn set_audio_info(&self, sample_rate: f32, channels: u8) -> Result<(), AudioSinkError> {
        let audio_info = gst_audio::AudioInfo::builder(
            gst_audio::AUDIO_FORMAT_F32,
            sample_rate as u32,
            channels.into(),
        )
        .build()
        .map_err(|error| AudioSinkError::Backend(format!("AudioInfo failed: {error:?}")))?;
        self.appsrc.set_caps(audio_info.to_caps().ok().as_ref());
        *self.audio_info.borrow_mut() = Some(audio_info);
        Ok(())
    }

    fn set_channels_if_changed(&self, channels: u8) -> Result<(), AudioSinkError> {
        let curr_channels = if let Some(ch) = self.audio_info.borrow().as_ref() {
            ch.channels()
        } else {
            return Ok(());
        };
        if channels != curr_channels as u8 {
            self.set_audio_info(self.sample_rate.get(), channels)?;
        }
        Ok(())
    }
}

impl AudioSink for GStreamerAudioSink {
    fn init(
        &self,
        sample_rate: f32,
        graph_thread_channel: Sender<AudioRenderThreadMsg>,
    ) -> Result<(), AudioSinkError> {
        self.sample_rate.set(sample_rate);
        self.set_audio_info(sample_rate, 2)?;
        self.appsrc.set_format(gst::Format::Time);

        // Allow only a single chunk.
        self.appsrc.set_max_bytes(1);

        let appsrc = self.appsrc.clone();
        Builder::new()
            .name("GstAppSrcCallbacks".to_owned())
            .spawn(move || {
                let need_data = move |_: &AppSrc, _: u32| {
                    if let Err(e) = graph_thread_channel
                        .send(AudioRenderThreadMsg::SinkNeedData)
                    {
                        log::warn!("Error sending need data event: {:?}", e);
                    }
                };
                appsrc.set_callbacks(AppSrcCallbacks::builder().need_data(need_data).build());
            })
            .unwrap();

        let appsrc = self.appsrc.as_ref().clone().upcast();
        let resample = gst::ElementFactory::make("audioresample")
            .build()
            .map_err(|error| AudioSinkError::Backend(format!("audioresample creation failed: {error:?}")))?;
        let convert = gst::ElementFactory::make("audioconvert")
            .build()
            .map_err(|error| AudioSinkError::Backend(format!("audioconvert creation failed: {error:?}")))?;
        let sink = gst::ElementFactory::make("autoaudiosink")
            .build()
            .map_err(|error| AudioSinkError::Backend(format!("autoaudiosink creation failed: {error:?}")))?;
        self.pipeline
            .add_many(&[&appsrc, &resample, &convert, &sink])
            .map_err(|error| AudioSinkError::Backend(error.to_string()))?;
        gst::Element::link_many(&[&appsrc, &resample, &convert, &sink])
            .map_err(|error| AudioSinkError::Backend(error.to_string()))?;

        Ok(())
    }

    fn init_stream(
        &self,
        channels: u8,
        sample_rate: f32,
        socket: Box<dyn MediaSocket>,
    ) -> Result<(), AudioSinkError> {
        self.sample_rate.set(sample_rate);
        self.set_audio_info(sample_rate, channels)?;
        self.appsrc.set_format(gst::Format::Time);

        // Do not set max bytes or callback, we will push as needed

        let appsrc = self.appsrc.as_ref().clone().upcast();
        let convert = gst::ElementFactory::make("audioconvert")
            .build()
            .map_err(|error| AudioSinkError::Backend(format!("audioconvert creation failed: {error:?}")))?;
        let sink = socket
            .as_any()
            .downcast_ref::<GstreamerMediaSocket>()
            .unwrap()
            .proxy_sink()
            .clone();

        self.pipeline
            .add_many(&[&appsrc, &convert, &sink])
            .map_err(|error| AudioSinkError::Backend(error.to_string()))?;
        gst::Element::link_many(&[&appsrc, &convert, &sink])
            .map_err(|error| AudioSinkError::Backend(error.to_string()))?;

        Ok(())
    }

    fn play(&self) -> Result<(), AudioSinkError> {
        self.pipeline
            .set_state(gst::State::Playing)
            .map(|_| ())
            .map_err(|_| AudioSinkError::StateChangeFailed)
    }

    fn stop(&self) -> Result<(), AudioSinkError> {
        self.pipeline
            .set_state(gst::State::Paused)
            .map(|_| ())
            .map_err(|_| AudioSinkError::StateChangeFailed)
    }

    fn has_enough_data(&self) -> bool {
        self.appsrc.current_level_bytes() >= self.appsrc.max_bytes()
    }

    fn push_data(&self, mut chunk: Chunk) -> Result<(), AudioSinkError> {
        if let Some(block) = chunk.blocks.get(0) {
            self.set_channels_if_changed(block.chan_count())?;
        }

        let sample_rate = self.sample_rate.get() as u64;
        let audio_info = self.audio_info.borrow();
        let audio_info = audio_info.as_ref().unwrap();
        let channels = audio_info.channels();
        let bpf = audio_info.bpf() as usize;
        assert_eq!(bpf, 4 * channels as usize);
        let n_samples = FRAMES_PER_BLOCK.0;
        let buf_size = (n_samples as usize) * (bpf);
        let mut buffer = gst::Buffer::with_size(buf_size).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            let mut sample_offset = self.sample_offset.get();
            // Calculate the current timestamp (PTS) and the next one,
            // and calculate the duration from the difference instead of
            // simply the number of samples to prevent rounding errors
            let pts = gst::ClockTime::from_nseconds(
                sample_offset
                    .mul_div_floor(gst::ClockTime::SECOND.nseconds(), sample_rate)
                    .unwrap(),
            );
            let next_pts: gst::ClockTime = gst::ClockTime::from_nseconds(
                (sample_offset + n_samples)
                    .mul_div_floor(gst::ClockTime::SECOND.nseconds(), sample_rate)
                    .unwrap(),
            );
            buffer.set_pts(Some(pts));
            buffer.set_duration(next_pts - pts);

            // sometimes nothing reaches the output
            if chunk.len() == 0 {
                chunk.blocks.push(Default::default());
                chunk.blocks[0].repeat(channels as u8);
            }
            debug_assert!(chunk.len() == 1);
            let mut data = chunk.blocks[0].interleave();
            let data = data.as_mut_byte_slice().expect("casting failed");

            // XXXManishearth if we have a safe way to convert
            // from Box<[f32]> to Box<[u8]> (similarly for Vec)
            // we can use Buffer::from_slice instead
            buffer.copy_from_slice(0, data).expect("copying failed");

            sample_offset += n_samples;
            self.sample_offset.set(sample_offset);
        }

        self.appsrc
            .push_buffer(buffer)
            .map(|_| ())
            .map_err(|_| AudioSinkError::BufferPushFailed)
    }

    fn set_eos_callback(&self, _: Box<dyn Fn(Box<dyn AsRef<[f32]>>) + Send + Sync + 'static>) {}
}

impl Drop for GStreamerAudioSink {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
