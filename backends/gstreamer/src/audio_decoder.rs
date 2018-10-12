use super::gst_app::{AppSink, AppSinkCallbacks, AppSrc};
use super::gst_audio;
use super::BackendError;
use byte_slice_cast::*;
use gst::buffer::{MappedBuffer, Readable};
use gst::prelude::*;
use gst::{self, MessageView};
use servo_media_audio::decoder::{AudioDecoder, AudioDecoderCallbacks, AudioDecoderOptions};
use std::io::Cursor;
use std::io::Read;
use std::sync::{mpsc, Arc, Mutex};

pub struct GStreamerAudioDecoderProgress(MappedBuffer<Readable>);

impl AsRef<[f32]> for GStreamerAudioDecoderProgress {
    fn as_ref(&self) -> &[f32] {
        self.0.as_ref().as_slice_of::<f32>().unwrap()
    }
}

pub struct GStreamerAudioDecoder {}

impl GStreamerAudioDecoder {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioDecoder for GStreamerAudioDecoder {
    type Error = BackendError;
    fn decode(
        &self,
        data: Vec<u8>,
        callbacks: AudioDecoderCallbacks<BackendError>,
        options: Option<AudioDecoderOptions>,
    ) {
        let pipeline = gst::Pipeline::new(None);
        let callbacks = Arc::new(callbacks);

        let appsrc = match gst::ElementFactory::make("appsrc", None) {
            Some(appsrc) => appsrc,
            None => return callbacks.error(BackendError::ElementCreationFailed("appsrc")),
        };

        let decodebin = match gst::ElementFactory::make("decodebin", None) {
            Some(decodebin) => decodebin,
            None => return callbacks.error(BackendError::ElementCreationFailed("decodebin")),
        };

        // decodebin uses something called a "sometimes-pad", which is basically
        // a pad that will show up when a certain condition is met,
        // in decodebins case that is media being decoded
        if let Err(e) = pipeline.add_many(&[&appsrc, &decodebin]) {
            return callbacks.error(BackendError::PipelineFailed(e.0));
        }

        if let Err(e) = gst::Element::link_many(&[&appsrc, &decodebin]) {
            return callbacks.error(BackendError::PipelineFailed(e.0));
        }

        let appsrc = appsrc.downcast::<AppSrc>().unwrap();

        let options = options.unwrap_or_default();

        let (sender, receiver) = mpsc::channel();
        let sender = Arc::new(Mutex::new(sender));

        let pipeline_ = pipeline.downgrade();
        let callbacks_ = callbacks.clone();
        let sender_ = sender.clone();
        // Initial pipeline looks like
        //
        // appsrc ! decodebin2! ...
        //
        // We plug in the second part of the pipeline, including the deinterleave element,
        // once the media starts being decoded.
        decodebin.connect_pad_added(move |_, src_pad| {
            // A decodebin pad was added, if this is an audio file,
            // plug in a deinterleave element to separate each planar channel.
            //
            // Sub pipeline looks like
            //
            // ... decodebin2 ! audioconvert ! audioresample ! capsfilter ! deinterleave ...
            //
            // deinterleave also uses a sometime-pad, so we need to wait until
            // a pad for a planar channel is added to plug in the last part of
            // the pipeline, with the appsink that will be pulling the data from
            // each channel.

            let callbacks = &callbacks_;
            let sender = &sender_;
            let pipeline = match pipeline_.upgrade() {
                Some(pipeline) => pipeline,
                None => {
                    callbacks.error(BackendError::PipelineFailed("upgrade"));
                    let _ = sender.lock().unwrap().send(());
                    return;
                }
            };

            let (is_audio, caps) = {
                let media_type = src_pad.get_current_caps().and_then(|caps| {
                    caps.get_structure(0).map(|s| {
                        let name = s.get_name();
                        (name.starts_with("audio/"), caps.clone())
                    })
                });

                match media_type {
                    None => {
                        callbacks.error(BackendError::Caps("Failed to get media type from pad"));
                        let _ = sender.lock().unwrap().send(());
                        return;
                    }
                    Some(media_type) => media_type,
                }
            };

            if !is_audio {
                callbacks.error(BackendError::InvalidMediaFormat);
                let _ = sender.lock().unwrap().send(());
                return;
            }

            let sample_audio_info = match gst_audio::AudioInfo::from_caps(&caps) {
                Some(sample_audio_info) => sample_audio_info,
                None => {
                    callbacks.error(BackendError::AudioInfoFailed);
                    let _ = sender.lock().unwrap().send(());
                    return;
                }
            };
            let channels = sample_audio_info.channels();
            callbacks.ready(channels);

            let insert_deinterleave = || -> Result<(), BackendError> {
                let convert = gst::ElementFactory::make("audioconvert", None)
                    .ok_or(BackendError::ElementCreationFailed("audioconvert"))?;
                let resample = gst::ElementFactory::make("audioresample", None)
                    .ok_or(BackendError::ElementCreationFailed("audioresample"))?;
                let filter = gst::ElementFactory::make("capsfilter", None)
                    .ok_or(BackendError::ElementCreationFailed("capsfilter"))?;
                let deinterleave = gst::ElementFactory::make("deinterleave", Some("deinterleave"))
                    .ok_or(BackendError::ElementCreationFailed("deinterleave"))?;

                deinterleave
                    .set_property("keep-positions", &true.to_value())
                    .map_err(|e| BackendError::SetPropertyFailed(e.0))?;
                let pipeline_ = pipeline.downgrade();
                let callbacks_ = callbacks.clone();
                deinterleave.connect_pad_added(move |_, src_pad| {
                    // A new pad for a planar channel was added in deinterleave.
                    // Plug in an appsink so we can pull the data from each channel.
                    //
                    // The end of the pipeline looks like:
                    //
                    // ... deinterleave ! queue ! appsink.
                    let callbacks = &callbacks_;
                    let pipeline = match pipeline_.upgrade() {
                        Some(pipeline) => pipeline,
                        None => return callbacks.error(BackendError::PipelineFailed("upgrade")),
                    };
                    let insert_sink = || -> Result<(), BackendError> {
                        let queue = gst::ElementFactory::make("queue", None)
                            .ok_or(BackendError::ElementCreationFailed("queue"))?;
                        let sink = gst::ElementFactory::make("appsink", None)
                            .ok_or(BackendError::ElementCreationFailed("appsink"))?;
                        let appsink = sink.clone().dynamic_cast::<AppSink>().unwrap();
                        sink.set_property("sync", &false.to_value())
                            .map_err(|e| BackendError::SetPropertyFailed(e.0))?;

                        let callbacks_ = callbacks.clone();
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
                                        callbacks_.error(BackendError::InvalidSample);
                                        return gst::FlowReturn::Error;
                                    };

                                    let caps = if let Some(caps) = sample.get_caps() {
                                        caps
                                    } else {
                                        callbacks_.error(BackendError::Caps(
                                            "Could not get caps from sample",
                                        ));
                                        return gst::FlowReturn::Error;
                                    };

                                    let audio_info = if let Some(audio_info) =
                                        gst_audio::AudioInfo::from_caps(&caps)
                                    {
                                        audio_info
                                    } else {
                                        callbacks_.error(BackendError::AudioInfoFailed);
                                        return gst::FlowReturn::Error;
                                    };
                                    assert_eq!(audio_info.channels(), 1);
                                    let positions = if let Some(positions) = audio_info.positions()
                                    {
                                        positions
                                    } else {
                                        callbacks_.error(BackendError::AudioInfoFailed);
                                        return gst::FlowReturn::Error;
                                    };

                                    for position in positions.iter() {
                                        let buffer = buffer.clone();
                                        let map =
                                            if let Ok(map) = buffer.into_mapped_buffer_readable() {
                                                map
                                            } else {
                                                callbacks_.error(BackendError::BufferReadError);
                                                return gst::FlowReturn::Error;
                                            };
                                        let progress = Box::new(GStreamerAudioDecoderProgress(map));
                                        let channel = position.to_mask() as u32;
                                        callbacks_.progress(progress, channel);
                                    }

                                    gst::FlowReturn::Ok
                                })
                                .build(),
                        );

                        let elements = &[&queue, &sink];
                        pipeline
                            .add_many(elements)
                            .map_err(|e| BackendError::PipelineFailed(e.0))?;
                        gst::Element::link_many(elements)
                            .map_err(|e| BackendError::PipelineFailed(e.0))?;

                        for e in elements {
                            e.sync_state_with_parent()
                                .map_err(|e| BackendError::PipelineFailed(e.0))?;
                        }

                        let sink_pad = queue
                            .get_static_pad("sink")
                            .ok_or(BackendError::GetStaticPadFailed("sink"))?;
                        src_pad
                            .link(&sink_pad)
                            .into_result()
                            .map(|_| ())
                            .map_err(|_| BackendError::PadLinkFailed)
                    };

                    if let Err(e) = insert_sink() {
                        callbacks.error(e);
                    }
                });

                let audio_info = gst_audio::AudioInfo::new(
                    gst_audio::AUDIO_FORMAT_F32,
                    options.sample_rate as u32,
                    channels,
                ).build()
                    .ok_or(BackendError::AudioInfoFailed)?;
                let caps = audio_info.to_caps().ok_or(BackendError::AudioInfoFailed)?;
                filter
                    .set_property("caps", &caps.to_value())
                    .map_err(|_| BackendError::SetPropertyFailed("caps"))?;

                let elements = &[&convert, &resample, &filter, &deinterleave];
                pipeline
                    .add_many(elements)
                    .map_err(|e| BackendError::PipelineFailed(e.0))?;
                gst::Element::link_many(elements).map_err(|e| BackendError::PipelineFailed(e.0))?;

                for e in elements {
                    e.sync_state_with_parent()
                        .map_err(|e| BackendError::PipelineFailed(e.0))?;
                }

                let sink_pad = convert
                    .get_static_pad("sink")
                    .ok_or(BackendError::GetStaticPadFailed("sink"))?;
                src_pad
                    .link(&sink_pad)
                    .into_result()
                    .map(|_| ())
                    .map_err(|_| BackendError::PadLinkFailed)
            };

            if let Err(e) = insert_deinterleave() {
                callbacks.error(e);
                let _ = sender.lock().unwrap().send(());
            }
        });

        appsrc.set_property_format(gst::Format::Bytes);
        appsrc.set_property_block(true);

        let bus = match pipeline.get_bus() {
            Some(bus) => bus,
            None => {
                callbacks.error(BackendError::PipelineFailed(
                    "Pipeline without bus. Shouldn't happen!",
                ));
                let _ = sender.lock().unwrap().send(());
                return;
            }
        };

        let callbacks_ = callbacks.clone();
        bus.set_sync_handler(move |_, msg| {
            match msg.view() {
                MessageView::Error(e) => {
                    callbacks_.error(BackendError::PipelineBusError(
                        e.get_debug().unwrap_or("Unknown".to_owned()),
                    ));
                    let _ = sender.lock().unwrap().send(());
                }
                MessageView::Eos(_) => {
                    callbacks_.eos();
                    let _ = sender.lock().unwrap().send(());
                }
                _ => (),
            }
            gst::BusSyncReply::Drop
        });

        if pipeline
            .set_state(gst::State::Playing)
            .into_result()
            .is_err()
        {
            callbacks.error(BackendError::StateChangeFailed);
            return;
        }

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

        // Wait until we get an error or EOS.
        receiver.recv().unwrap();
        let _ = pipeline.set_state(gst::State::Null);
    }
}
