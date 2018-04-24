use super::gst;
use gst::prelude::*;
use AudioStream;

use super::src_element::app_src_oscillator;

// XXX Define own error type.

pub struct GStreamerAudioStream {
    pipeline: gst::Pipeline,
}

impl GStreamerAudioStream {
    pub fn new() -> Result<Self, ()> {
        if let Some(category) = gst::DebugCategory::get("openslessink") {
            category.set_threshold(gst::DebugLevel::Trace);
        }
        gst::init().map_err(|_| ())?;

        let src = app_src_oscillator()?;
        let convert = gst::ElementFactory::make("audioconvert", None).ok_or(())?;
        let sink = gst::ElementFactory::make("autoaudiosink", None).ok_or(())?;
        let pipeline = gst::Pipeline::new(None);
        pipeline.add_many(&[&src, &convert, &sink]).map_err(|_| ())?;
        gst::Element::link_many(&[&src, &convert, &sink]).map_err(|_| ())?;
        Ok(Self { pipeline })
    }
}

impl AudioStream for GStreamerAudioStream {
    fn play(&self) {
        let _ = self.pipeline.set_state(gst::State::Playing);
    }

    fn stop(&self) {
        let _ = self.pipeline.set_state(gst::State::Paused);
    }
}

impl Drop for GStreamerAudioStream {
    fn drop(&mut self) {
        self.stop();
    }
}
