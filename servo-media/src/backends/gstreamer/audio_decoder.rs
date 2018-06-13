use super::gst_app::AppSrc;
use audio::decoder::AudioDecoder;
use gst;
use gst::prelude::*;
use std::thread::Builder;

pub struct GStreamerAudioDecoder {}

impl GStreamerAudioDecoder {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioDecoder for GStreamerAudioDecoder {
    fn decode(&self, mut data: Vec<u8>) -> Result<(), ()> {
        let pipeline = gst::Pipeline::new(None);
        let appsrc = gst::ElementFactory::make("appsrc", None).ok_or(())?;
        let decodebin = gst::ElementFactory::make("decodebin", None).ok_or(())?;
        // decodebin uses something called a "sometimes-pad", which is basically
        // a pad that will show up when a certain condition is met,
        // in decodebins case that is media being decoded
        pipeline.add_many(&[&appsrc, &decodebin]).map_err(|_| ())?;
        gst::Element::link_many(&[&appsrc, &decodebin]).map_err(|_| ())?;

        let pipeline_ = pipeline.clone();
        decodebin.connect_pad_added(move |_, src_pad| {
            let pipeline = &pipeline_;

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
                        // XXX reject Future.
                        return;
                    }
                    Some(media_type) => media_type,
                }
            };

            let insert_sink = || -> Result<(), ()> {
                let queue = gst::ElementFactory::make("queue", None).ok_or(())?;
                let convert = gst::ElementFactory::make("audioconvert", None).ok_or(())?;
                let resample = gst::ElementFactory::make("audioresample", None).ok_or(())?;
                // XXX Use audiosink for now. This should end up being an appsink.
                let sink = gst::ElementFactory::make("autoaudiosink", None).ok_or(())?;

                let elements = &[&queue, &convert, &resample, &sink];
                pipeline.add_many(elements).map_err(|_| ())?;
                gst::Element::link_many(elements).map_err(|_| ())?;

                for e in elements {
                    e.sync_state_with_parent().map_err(|_| ())?;
                }

                let sink_pad = queue.get_static_pad("sink").expect("queue has no sinkpad");
                src_pad
                    .link(&sink_pad)
                    .into_result()
                    .map(|_| ())
                    .map_err(|_| ())
            };

            if !is_audio || insert_sink().is_err() {
                // XXX reject Future.
                return;
            }
        });

        let appsrc = appsrc.downcast::<AppSrc>().map_err(|_| ())?;
        appsrc.set_property_format(gst::Format::Time);
        appsrc.set_property_block(true);

        let _ = pipeline.set_state(gst::State::Playing);

        // We push data into the appsrc in a different thread so we
        // can get messages from the bus.

        let appsrc_ = appsrc.clone();
        Builder::new()
            .name("Decoder data loop".to_owned())
            .spawn(move || {
                let mut offset = 0;
                let max_bytes = appsrc_.get_max_bytes() as usize;
                while offset < data.len() {
                    let data_left = data.len() - offset;
                    let buffer_size = if data_left < max_bytes {
                        data_left
                    } else {
                        max_bytes
                    };
                    let mut buffer = gst::Buffer::with_size(buffer_size).unwrap();
                    {
                        let buffer = buffer.get_mut().unwrap();
                        let next_offset = offset + buffer_size;
                        let data = data.as_mut_slice();
                        buffer
                            .copy_from_slice(0, &data[offset..next_offset])
                            .expect("copying failed");
                        offset = next_offset;
                    }
                    appsrc_
                        .push_buffer(buffer)
                        .into_result()
                        .expect("pushing buffer failed");
                }
            })
            .unwrap();

        let bus = pipeline
            .get_bus()
            .expect("Pipeline without bus. Shouldn't happen!");

        while let Some(msg) = bus.timed_pop(gst::CLOCK_TIME_NONE) {
            use gst::MessageView;

            match msg.view() {
                MessageView::Eos(..) => break,
                MessageView::Error(_) => {
                    pipeline
                        .set_state(gst::State::Null)
                        .into_result()
                        .expect("Setting pipeline state failed");
                    // XXX Reject Future.
                    break;
                }
                _ => (),
            }
        }

        let _ = pipeline.set_state(gst::State::Null);

        Ok(())
    }
}
