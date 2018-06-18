use super::gst_app::{AppSink, AppSinkCallbacks, AppSrc};
use audio::decoder::{AudioDecoder, AudioDecoderMsg};
use byte_slice_cast::*;
use gst;
use gst::prelude::*;
use std::io::Cursor;
use std::io::Read;
use sync::mpsc::Sender;
use sync::{Arc, Mutex};

pub struct GStreamerAudioDecoder {}

impl GStreamerAudioDecoder {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioDecoder for GStreamerAudioDecoder {
    fn decode(&self, data: Vec<u8>, sender: Sender<AudioDecoderMsg>) {
        let pipeline = gst::Pipeline::new(None);
        let sender = Arc::new(Mutex::new(sender));

        let pipeline_ = pipeline.clone();
        let sender_ = sender.clone();
        let error = move || {
            let _ = sender_.lock().unwrap().send(AudioDecoderMsg::Error);
            let _ = pipeline_.set_state(gst::State::Null);
        };

        let appsrc = match gst::ElementFactory::make("appsrc", None) {
            Some(appsrc) => appsrc,
            None => return error(),
        };

        let decodebin = match gst::ElementFactory::make("decodebin", None) {
            Some(decodebin) => decodebin,
            None => return error(),
        };

        // decodebin uses something called a "sometimes-pad", which is basically
        // a pad that will show up when a certain condition is met,
        // in decodebins case that is media being decoded
        if pipeline.add_many(&[&appsrc, &decodebin]).is_err() {
            return error();
        }

        if gst::Element::link_many(&[&appsrc, &decodebin]).is_err() {
            return error();
        }

        let appsrc = match appsrc.downcast::<AppSrc>() {
            Ok(appsrc) => appsrc,
            Err(_) => {
                return error();
            }
        };

        let pipeline_ = pipeline.clone();
        let sender_ = sender.clone();
        decodebin.connect_pad_added(move |_, src_pad| {
            // We only want a sink per audio stream.
            if src_pad.is_linked() {
                return;
            }

            let pipeline = &pipeline_;
            let sender = &sender_;

            let (is_audio, _) = {
                let media_type = src_pad.get_current_caps().and_then(|caps| {
                    caps.get_structure(0).map(|s| {
                        let name = s.get_name();
                        (name.starts_with("audio/"), name.starts_with("video/"))
                    })
                });

                match media_type {
                    None => {
                        eprintln!("Failed to get media type from pad {}", src_pad.get_name());
                        return error();
                    }
                    Some(media_type) => media_type,
                }
            };

            if !is_audio {
                return error();
            }

            let insert_sink = || -> Result<(), ()> {
                let queue = gst::ElementFactory::make("queue", None).ok_or(())?;
                let convert = gst::ElementFactory::make("audioconvert", None).ok_or(())?;
                let resample = gst::ElementFactory::make("audioresample", None).ok_or(())?;
                let sink = gst::ElementFactory::make("appsink", None).ok_or(())?;
                let appsink = sink.clone().dynamic_cast::<AppSink>().map_err(|_| ())?;

                let pipeline_ = pipeline.clone();
                let pipeline__ = pipeline.clone();
                let sender_ = sender.clone();
                let sender__ = sender.clone();
                appsink.set_callbacks(
                    AppSinkCallbacks::new()
                        .new_sample(move |appsink| {
                            let sample = match appsink.pull_sample() {
                                None => {
                                    return gst::FlowReturn::Eos;
                                }
                                Some(sample) => sample,
                            };

                            let buffer = if let Some(buffer) = sample.get_buffer() {
                                buffer
                            } else {
                                let _ = sender_.lock().unwrap().send(AudioDecoderMsg::Error);
                                let _ = pipeline_.set_state(gst::State::Null);
                                return gst::FlowReturn::Error;
                            };

                            let mut progress: Vec<f32> = vec![0.; buffer.get_size() / 4];
                            if buffer
                                .copy_to_slice(
                                    0,
                                    progress.as_mut_slice().as_mut_byte_slice().unwrap(),
                                )
                                .is_err()
                            {
                                let _ = sender_.lock().unwrap().send(AudioDecoderMsg::Error);
                                let _ = pipeline_.set_state(gst::State::Null);
                                return gst::FlowReturn::Error;
                            }

                            let _ = sender_
                                .lock()
                                .unwrap()
                                .send(AudioDecoderMsg::Progress(progress));

                            gst::FlowReturn::Ok
                        })
                        .eos(move |_| {
                            let _ = sender__.lock().unwrap().send(AudioDecoderMsg::Eos);
                            let _ = pipeline__.set_state(gst::State::Null);
                        })
                        .build(),
                );

                let elements = &[&queue, &convert, &resample, &sink];
                pipeline.add_many(elements).map_err(|_| ())?;
                gst::Element::link_many(elements).map_err(|_| ())?;

                for e in elements {
                    e.sync_state_with_parent().map_err(|_| ())?;
                }

                let sink_pad = queue.get_static_pad("sink").ok_or(())?;
                src_pad
                    .link(&sink_pad)
                    .into_result()
                    .map(|_| ())
                    .map_err(|_| ())
            };

            if insert_sink().is_err() {
                error();
            }
        });

        appsrc.set_property_format(gst::Format::Bytes);
        appsrc.set_property_block(true);

        let _ = pipeline.set_state(gst::State::Playing);

        let max_bytes = appsrc.get_max_bytes() as usize;
        let data_len = data.len();
        let mut reader = Cursor::new(data);
        while (reader.position() as usize) < data_len {
            let data_left = data_len - reader.position() as usize;
            let buffer_size = if data_left < max_bytes {
                data_left
            } else {
                max_bytes
            };
            let mut buffer = gst::Buffer::with_size(buffer_size).unwrap();
            {
                let buffer = buffer.get_mut().unwrap();
                let mut map = buffer.map_writable().unwrap();
                let mut buffer = map.as_mut_slice();
                let _ = reader.read(&mut buffer);
            }
            let _ = appsrc.push_buffer(buffer);
        }
        let _ = appsrc.end_of_stream();
    }
}
