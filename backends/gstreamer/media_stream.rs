use BACKEND_BASE_TIME;
use glib::prelude::*;
use gst;
use gst::prelude::*;
use servo_media_streams::{MediaOutput, MediaStream};
use std::any::Any;

lazy_static! {
    static ref RTP_CAPS_OPUS: gst::Caps = {
        gst::Caps::new_simple(
            "application/x-rtp",
            &[
                ("media", &"audio"),
                ("encoding-name", &"OPUS"),
            ],
        )
    };
    static ref RTP_CAPS_VP8: gst::Caps = {
        gst::Caps::new_simple(
            "application/x-rtp",
            &[
                ("media", &"video"),
                ("encoding-name", &"VP8"),
            ],
        )
    };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StreamType {
    Audio,
    Video,
}

pub struct GStreamerMediaStream {
    type_: StreamType,
    elements: Vec<gst::Element>,
    pipeline: Option<gst::Pipeline>,
}

impl MediaStream for GStreamerMediaStream {
    fn as_any(&self) -> &Any {
        self
    }

    fn as_mut_any(&mut self) -> &mut Any {
        self
    }
}

impl GStreamerMediaStream {
    pub fn type_(&self) -> StreamType {
        self.type_
    }

    pub fn caps(&self) -> &gst::Caps {
        match self.type_ {
            StreamType::Audio => &*RTP_CAPS_OPUS,
            StreamType::Video => &*RTP_CAPS_VP8,
        }
    }

    pub fn caps_with_payload(&self, payload: i32) -> gst::Caps {
        match self.type_ {
            StreamType::Audio => {
                gst::Caps::new_simple(
                    "application/x-rtp",
                    &[
                        ("media", &"audio"),
                        ("encoding-name", &"OPUS"),
                        ("payload", &(payload)),
                    ],
                )
            }
            StreamType::Video => {
                gst::Caps::new_simple(
                    "application/x-rtp",
                    &[
                        ("media", &"video"),
                        ("encoding-name", &"VP8"),
                        ("payload", &(payload)),
                    ],
                )
            }
        }
    }

    pub fn insert_capsfilter(&mut self) {
        assert!(self.pipeline.is_none());
        let capsfilter = gst::ElementFactory::make("capsfilter", None).unwrap();
        capsfilter.set_property("caps", self.caps()).unwrap();
        self.elements.push(capsfilter);
    }

    pub fn src_element(&self) -> gst::Element {
        self.elements.last().unwrap().clone()
    }

    pub fn attach_to_pipeline(&mut self, pipeline: &gst::Pipeline) {
        assert!(self.pipeline.is_none());
        let elements: Vec<_> = self.elements.iter().collect();
        pipeline.add_many(&elements[..]).unwrap();
        gst::Element::link_many(&elements[..]).unwrap();
        for element in elements {
            element.sync_state_with_parent().unwrap();
        }
        self.pipeline = Some(pipeline.clone());
    }

    pub fn pipeline_or_new(&mut self) -> gst::Pipeline {
        if let Some(ref pipeline) = self.pipeline {
            pipeline.clone()
        } else {
            let pipeline = gst::Pipeline::new("gstreamermediastream fresh pipeline");
            let clock = gst::SystemClock::obtain();
            pipeline.set_start_time(gst::ClockTime::none());
            pipeline.set_base_time(*BACKEND_BASE_TIME);
            pipeline.use_clock(Some(&clock));
            self.attach_to_pipeline(&pipeline);
            pipeline
        }
    }

    pub fn create_video() -> GStreamerMediaStream {
        let videotestsrc = gst::ElementFactory::make("videotestsrc", None).unwrap();
        videotestsrc.set_property_from_str("pattern", "ball");
        videotestsrc
            .set_property("is-live", &true)
            .expect("videotestsrc doesn't have expected 'is-live' property");

        Self::create_video_from_encoded(videotestsrc)
    }

    pub fn create_video_from(source: gst::Element) -> GStreamerMediaStream {
        let videoconvert = gst::ElementFactory::make("videoconvert", None).unwrap();
        let queue = gst::ElementFactory::make("queue", None).unwrap();

        GStreamerMediaStream {
            type_: StreamType::Video,
            elements: vec![source, videoconvert, queue],
            pipeline: None,
        }
    }

    pub fn create_video_from_encoded(source: gst::Element) -> GStreamerMediaStream {
        let videoconvert = gst::ElementFactory::make("videoconvert", None).unwrap();
        let queue = gst::ElementFactory::make("queue", None).unwrap();
        let vp8enc = gst::ElementFactory::make("vp8enc", None).unwrap();

        vp8enc
            .set_property("deadline", &1i64)
            .expect("vp8enc doesn't have expected 'deadline' property");

        let rtpvp8pay = gst::ElementFactory::make("rtpvp8pay", None).unwrap();
        let queue2 = gst::ElementFactory::make("queue", None).unwrap();

        GStreamerMediaStream {
            type_: StreamType::Video,
            elements: vec![source, videoconvert, queue, vp8enc, rtpvp8pay, queue2],
            pipeline: None,
        }
    }

    pub fn create_audio() -> GStreamerMediaStream {
        let audiotestsrc = gst::ElementFactory::make("audiotestsrc", None).unwrap();
        audiotestsrc.set_property_from_str("wave", "sine");
        audiotestsrc
            .set_property("is-live", &true)
            .expect("audiotestsrc doesn't have expected 'is-live' property");

        Self::create_audio_from_encoded(audiotestsrc)
    }

    pub fn create_audio_from(source: gst::Element) -> GStreamerMediaStream {
        let queue = gst::ElementFactory::make("queue", None).unwrap();
        let audioconvert = gst::ElementFactory::make("audioconvert", None).unwrap();
        let audioresample = gst::ElementFactory::make("audioresample", None).unwrap();
        let queue2 = gst::ElementFactory::make("queue", None).unwrap();

        GStreamerMediaStream {
            type_: StreamType::Audio,
            elements: vec![
                source,
                queue,
                audioconvert,
                audioresample,
                queue2,
            ],
            pipeline: None,
        }
    }

    pub fn create_audio_from_encoded(source: gst::Element) -> GStreamerMediaStream {
        let queue = gst::ElementFactory::make("queue", None).unwrap();
        let audioconvert = gst::ElementFactory::make("audioconvert", None).unwrap();
        let audioresample = gst::ElementFactory::make("audioresample", None).unwrap();
        let queue2 = gst::ElementFactory::make("queue", None).unwrap();
        let opusenc = gst::ElementFactory::make("opusenc", None).unwrap();
        let rtpopuspay = gst::ElementFactory::make("rtpopuspay", None).unwrap();
        let queue3 = gst::ElementFactory::make("queue", None).unwrap();

        GStreamerMediaStream {
            type_: StreamType::Audio,
            elements: vec![
                source,
                queue,
                audioconvert,
                audioresample,
                queue2,
                opusenc,
                rtpopuspay,
                queue3,
            ],
            pipeline: None,
        }
    }
}

pub struct MediaSink {
    streams: Vec<Box<MediaStream>>,
}

impl MediaSink {
    pub fn new() -> Self {
        MediaSink { streams: vec![] }
    }
}

impl MediaOutput for MediaSink {
    fn add_stream(&mut self, mut stream: Box<MediaStream>) {
        {
            let stream = stream
                .as_mut_any()
                .downcast_mut::<GStreamerMediaStream>()
                .unwrap();
            let pipeline = stream.pipeline_or_new();
            let last_element = stream.elements.last();
            let last_element = last_element.as_ref().unwrap();
            let sink = match stream.type_ {
                StreamType::Audio => "autoaudiosink",
                StreamType::Video => "autovideosink",
            };
            let sink = gst::ElementFactory::make(sink, None).unwrap();
            pipeline.add(&sink).unwrap();
            gst::Element::link_many(&[last_element, &sink][..]).unwrap();

            pipeline.set_state(gst::State::Playing).unwrap();
            sink.sync_state_with_parent().unwrap();
            // gst::debug_bin_to_dot_file(&pipeline,  gstreamer::DebugGraphDetails::ALL, ::std::path::Path::new("dot.dot"));
        }

        self.streams.push(stream);
    }
}
