use AudioStream;
use gst::prelude::*;
use super::gst;

use std::process;

// XXX Define own error type.

pub struct GStreamerAudioStream {
    pipeline: gst::Element,
}

impl GStreamerAudioStream {
    pub fn new() -> Result<Self, ()> {
        if let Some(category) = gst::DebugCategory::get("openslessink") {
            category.set_threshold(gst::DebugLevel::Trace);
        }
        let pipeline_str = "servoaudiosrc ! audioconvert ! autoaudiosink";

        let mut context = gst::ParseContext::new();
        let pipeline = match gst::parse_launch_full(
            &pipeline_str,
            Some(&mut context),
            gst::ParseFlags::NONE,
        ) {
            Ok(pipeline) => pipeline,
            Err(err) => {
                if let Some(gst::ParseError::NoSuchElement) = err.kind::<gst::ParseError>() {
                    println!("Missing element(s): {:?}", context.get_missing_elements());
                } else {
                    println!("Failed to parse pipeline: {}", err);
                }

                process::exit(-1)
            }
        };

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
