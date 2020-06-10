use media_stream::GStreamerMediaStream;
use servo_media_audio::block::{Block, FRAMES_PER_BLOCK_USIZE};
use servo_media_audio::AudioStreamReader;
use servo_media_streams::registry::{get_stream, MediaStreamId};
use std::sync::mpsc::{channel, Receiver};

use gst::prelude::*;
use gst::{Caps, Fraction};
use gst_audio::AUDIO_FORMAT_F32;
use byte_slice_cast::*;

pub struct GStreamerAudioStreamReader {
    rx: Receiver<Block>,
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

        let split = gst::ElementFactory::make("audiobuffersplit", None)
            .map_err(|_| "audiobuffersplit creation failed".to_owned())?;
        split
            .set_property("output-buffer-duration", &time_per_block)
            .map_err(|_| "setting duration failed".to_owned())?;
        let convert = gst::ElementFactory::make("audioconvert", None)
            .map_err(|_| "audioconvert creation failed".to_owned())?;
        let capsfilter = gst::ElementFactory::make("capsfilter", None)
            .map_err(|_| "capsfilter creation failed".to_owned())?;
        let caps = Caps::new_simple(
            "audio/x-raw",
            &[
                ("layout", &"non-interleaved"),
                ("format", &AUDIO_FORMAT_F32.to_string()),
                ("rate", &(sample_rate as i32)),
            ],
        );
        capsfilter.set_property("caps", &caps).unwrap();
        let sink = gst::ElementFactory::make("appsink", None)
            .map_err(|_| "appsink creation failed".to_owned())?;
        sink.set_property("sync", &false.to_value())
            .expect("appsink doesn't handle expected 'sync' property");

        let appsink = sink.clone().dynamic_cast::<gst_app::AppSink>().unwrap();

        let elements = [&element, &split, &convert, &capsfilter, &sink];
        pipeline
            .add_many(&elements[1..])
            .map_err(|_| "pipeline adding failed".to_owned())?;
        gst::Element::link_many(&elements).map_err(|_| "element linking failed".to_owned())?;
        for e in &elements {
            e.sync_state_with_parent().map_err(|e| e.to_string())?;
        }
        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::new()
                .new_sample(move |appsink| {
                    let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    let buffer = sample.get_buffer_owned().ok_or(gst::FlowError::Error)?;

                    let buffer = buffer.into_mapped_buffer_readable().map_err(|_| gst::FlowError::Error)?;
                    let floatref = buffer.as_slice().as_slice_of::<f32>().map_err(|_| gst::FlowError::Error)?;
                    
                    let block = Block::for_vec(floatref.into());
                    tx.send(block).map_err(|_| gst::FlowError::Error)?;
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );
        pipeline
            .set_state(gst::State::Playing)
            .map_err(|_| "pipeline state change failed".to_owned())?;
        Ok(Self { rx })
    }
}

impl AudioStreamReader for GStreamerAudioStreamReader {
    fn pull(&self) -> Block {
        self.rx.recv().unwrap()
    }
}
