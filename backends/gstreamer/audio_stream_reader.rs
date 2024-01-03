use crate::media_stream::GStreamerMediaStream;
use servo_media_audio::block::{Block, FRAMES_PER_BLOCK_USIZE};
use servo_media_audio::AudioStreamReader;
use servo_media_streams::registry::{get_stream, MediaStreamId};
use std::sync::mpsc::{channel, Receiver};

use byte_slice_cast::*;
use gst::prelude::*;
use gst::Fraction;
use gst_audio::AUDIO_FORMAT_F32;

pub struct GStreamerAudioStreamReader {
    rx: Receiver<Block>,
    pipeline: gst::Pipeline,
}

impl GStreamerAudioStreamReader {
    pub fn new(stream: MediaStreamId, sample_rate: f32) -> Result<Self, String> {
        let (tx, rx) = channel();
        let stream = get_stream(&stream).unwrap();
        let mut stream = stream.lock().unwrap();
        let g_stream = stream
            .as_mut_any()
            .downcast_mut::<GStreamerMediaStream>()
            .unwrap();
        let element = g_stream.src_element();
        let pipeline = g_stream.pipeline_or_new();
        drop(stream);
        let time_per_block = Fraction::new(FRAMES_PER_BLOCK_USIZE as i32, sample_rate as i32);

        // XXXManishearth this is only necessary because of an upstream
        // gstreamer bug. https://github.com/servo/media/pull/362#issuecomment-647947034
        let caps = gst_audio::AudioCapsBuilder::new()
            .layout(gst_audio::AudioLayout::Interleaved)
            .build();
        let capsfilter0 = gst::ElementFactory::make("capsfilter")
            .property("caps", caps)
            .build()
            .map_err(|error| format!("capsfilter creation failed: {error:?}"))?;

        let split = gst::ElementFactory::make("audiobuffersplit")
            .property("output-buffer-duration", time_per_block)
            .build()
            .map_err(|error| format!("audiobuffersplit creation failed: {error:?}"))?;
        let convert = gst::ElementFactory::make("audioconvert")
            .build()
            .map_err(|error| format!("audioconvert creation failed: {error:?}"))?;
        let caps = gst_audio::AudioCapsBuilder::new()
            .layout(gst_audio::AudioLayout::NonInterleaved)
            .format(AUDIO_FORMAT_F32)
            .rate(sample_rate as i32)
            .build();
        let capsfilter = gst::ElementFactory::make("capsfilter")
            .property("caps", caps)
            .build()
            .map_err(|error| format!("capsfilter creation failed: {error:?}"))?;
        let sink = gst::ElementFactory::make("appsink")
            .property("sync", false)
            .build()
            .map_err(|error| format!("appsink creation failed: {error:?}"))?;

        let appsink = sink.clone().dynamic_cast::<gst_app::AppSink>().unwrap();

        let elements = [&element, &capsfilter0, &split, &convert, &capsfilter, &sink];
        pipeline
            .add_many(&elements[1..])
            .map_err(|error| format!("pipeline adding failed: {error:?}"))?;
        gst::Element::link_many(&elements).map_err(|error| format!("element linking failed: {error:?}"))?;
        for e in &elements {
            e.sync_state_with_parent().map_err(|e| e.to_string())?;
        }
        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    let buffer = sample.buffer_owned().ok_or(gst::FlowError::Error)?;

                    let buffer = buffer
                        .into_mapped_buffer_readable()
                        .map_err(|_| gst::FlowError::Error)?;
                    let floatref = buffer
                        .as_slice()
                        .as_slice_of::<f32>()
                        .map_err(|_| gst::FlowError::Error)?;

                    let block = Block::for_vec(floatref.into());
                    tx.send(block).map_err(|_| gst::FlowError::Error)?;
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );
        Ok(Self { rx, pipeline })
    }
}

impl AudioStreamReader for GStreamerAudioStreamReader {
    fn pull(&self) -> Block {
        self.rx.recv().unwrap()
    }

    fn start(&self) {
        self.pipeline.set_state(gst::State::Playing).unwrap();
    }

    fn stop(&self) {
        self.pipeline.set_state(gst::State::Null).unwrap();
    }
}
