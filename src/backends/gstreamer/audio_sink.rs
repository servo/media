use super::gst;
use super::gst_app::{AppSrc, AppSrcCallbacks};
use super::gst_audio;
use audio::graph::AudioGraph;
use audio::sink::AudioSink;
use gst::prelude::*;
use std::sync::Arc;

// XXX Define own error type.

// Default values of properties
const DEFAULT_SAMPLES_PER_BUFFER: u32 = 1024;
const DEFAULT_FREQ: u32 = 440;
const DEFAULT_VOLUME: f64 = 0.8;
const DEFAULT_MUTE: bool = false;
const DEFAULT_IS_LIVE: bool = false;

// Property value storage
#[derive(Debug, Clone, Copy)]
struct Settings {
    samples_per_buffer: u32,
    freq: u32,
    volume: f64,
    mute: bool,
    is_live: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            samples_per_buffer: DEFAULT_SAMPLES_PER_BUFFER,
            freq: DEFAULT_FREQ,
            volume: DEFAULT_VOLUME,
            mute: DEFAULT_MUTE,
            is_live: DEFAULT_IS_LIVE,
        }
    }
}

pub struct GStreamerAudioSink {
    pipeline: gst::Pipeline,
}

impl GStreamerAudioSink {
    pub fn new() -> Self {
        Self {
            pipeline: gst::Pipeline::new(None),
        }
    }
}

impl AudioSink for GStreamerAudioSink {
    fn init(&self, graph: Arc<AudioGraph>) -> Result<(), ()> {
        if let Some(category) = gst::DebugCategory::get("openslessink") {
            category.set_threshold(gst::DebugLevel::Trace);
        }
        gst::init().map_err(|_| ())?;

        let src = gst::ElementFactory::make("appsrc", None).ok_or(())?;
        let src = src.downcast::<AppSrc>().map_err(|_| ())?;
        let info = gst_audio::AudioInfo::new(gst_audio::AUDIO_FORMAT_F32, 48000, 1)
            .build()
            .ok_or(())?;
        src.set_caps(&info.to_caps().unwrap());
        src.set_property_format(gst::Format::Time);
        let settings = Settings::default();
        let mut sample_offset = 0;
        let mut accumulator = 0.;
        let n_samples = settings.samples_per_buffer as u64;
        let buf_size = (n_samples as usize) * (info.bpf() as usize);
        let rate = info.rate();

        let graph_ = graph.clone();
        let need_data = move |app: &AppSrc, _bytes: u32| {
            let mut buffer = gst::Buffer::with_size(buf_size).unwrap();
            {
                let buffer = buffer.get_mut().unwrap();
                // Calculate the current timestamp (PTS) and the next one,
                // and calculate the duration from the difference instead of
                // simply the number of samples to prevent rounding errors
                let pts = sample_offset
                    .mul_div_floor(gst::SECOND_VAL, rate as u64)
                    .unwrap()
                    .into();
                let next_pts: gst::ClockTime = (sample_offset + n_samples)
                    .mul_div_floor(gst::SECOND_VAL, rate as u64)
                    .unwrap()
                    .into();
                buffer.set_pts(pts);
                buffer.set_duration(next_pts - pts);
                let mut map = buffer.map_writable().unwrap();
                let data = map.as_mut_slice();
                graph_.process(
                    data,
                    &mut accumulator,
                    settings.freq,
                    rate,
                    1,
                    settings.volume,
                );
                sample_offset += n_samples;
            }
            app.push_buffer(buffer);
        };
        src.set_callbacks(AppSrcCallbacks::new().need_data(need_data).build());

        let src = src.upcast();
        let convert = gst::ElementFactory::make("audioconvert", None).ok_or(())?;
        let sink = gst::ElementFactory::make("autoaudiosink", None).ok_or(())?;
        self.pipeline
            .add_many(&[&src, &convert, &sink])
            .map_err(|_| ())?;
        gst::Element::link_many(&[&src, &convert, &sink]).map_err(|_| ())?;

        Ok(())
    }

    fn play(&self) {
        let _ = self.pipeline.set_state(gst::State::Playing);
    }

    fn stop(&self) {
        let _ = self.pipeline.set_state(gst::State::Paused);
    }
}

impl Drop for GStreamerAudioSink {
    fn drop(&mut self) {
        self.stop();
    }
}
