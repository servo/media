extern crate gstreamer_audio as gst_audio;
extern crate gstreamer_base as gst_base;

// XXX not needed at some point.
extern crate byte_slice_cast;
extern crate num_traits;

pub mod src_element;

use AudioStream;
use gst;
use gst::MessageView;
use gst::prelude::*;
use ServoMediaBackend;

// XXX Define own error type.

pub struct GStreamerAudioStream {
    pipeline: gst::Pipeline,
}

impl GStreamerAudioStream {
    pub fn new() -> Result<Self, ()> {
        gst::init().map_err(|_| ())?;
        let src = gst::ElementFactory::make("servoaudiosrc", None).ok_or(())?;
        let convert = gst::ElementFactory::make("audioconvert", None).ok_or(())?;
        let sink = gst::ElementFactory::make("autoaudiosink", None).ok_or(())?;
        let pipeline = gst::Pipeline::new(None);
        pipeline.add_many(&[&src, &convert, &sink]).map_err(|_| ())?;
        gst::Element::link_many(&[&src, &convert, &sink]).map_err(|_| ())?;
        Ok(Self {
            pipeline
        })
    }
}

impl AudioStream for GStreamerAudioStream {
    fn play(&self) {
        if self.pipeline.set_state(gst::State::Playing) == gst::StateChangeReturn::Failure {
            eprintln!("Unable to set the pipeline to the playing state");
        }

        let bus = self.pipeline.get_bus().unwrap();

        while let Some(msg) = bus.timed_pop(gst::CLOCK_TIME_NONE) {
            match msg.view() {
                MessageView::Error(err) => {
                    println!(
                        "Error received from element {:?}: {} ({:?})",
                        err.get_src().map(|s| s.get_path_string()),
                        err.get_error(),
                        err.get_debug()
                    );
                    break;
                },
                MessageView::Eos(..) => {
                    println!("End-Of-Stream reached");
                    break;
                },
                MessageView::StateChanged(state_changed) => {
                    if state_changed.get_src().map(|s| s == self.pipeline).unwrap_or(false) {
                        let new_state = state_changed.get_current();
                        let old_state = state_changed.get_old();

                        println!(
                            "Pipeline state changed from {:?} to {:?}",
                            old_state,
                            new_state,
                        );
                    }
                },
                _ => (),
            }
        }
    }

    fn stop(&self) {
        self.pipeline.set_state(gst::State::Paused);
    }
}

impl Drop for GStreamerAudioStream {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

pub struct GStreamer {}

impl GStreamer {
    pub fn new() -> Self {
        Self {}
    }
}

impl ServoMediaBackend for GStreamer {
    fn version(&self) -> String {
        gst::init().unwrap();
        gst::version_string()
    }

    fn get_audio_stream(&self) -> Result<Box<AudioStream>, ()> {
        let stream = GStreamerAudioStream::new()?;
        Ok(Box::new(stream))
    }
}
