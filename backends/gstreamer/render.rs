use glib::prelude::*;
use gst;
use gst_app;
use gst_video;

use std::sync::Arc;

use servo_media_player::frame::{Buffer, Frame, FrameData};
use servo_media_player::PlayerError;

struct GStreamerBuffer {
    frame: gst_video::VideoFrame<gst_video::video_frame::Readable>,
}

impl Buffer for GStreamerBuffer {
    fn to_vec(&self) -> Result<FrameData, ()> {
        let data = self.frame.plane_data(0).ok_or_else(|| ())?;
        Ok(FrameData::Raw(Arc::new(data.to_vec())))
    }
}

pub struct GStreamerRender();

impl GStreamerRender {
    pub fn new() -> Self {
        GStreamerRender {}
    }

    pub fn get_frame_from_sample(&self, sample: &gst::Sample) -> Result<Frame, ()> {
        let buffer = sample.get_buffer().ok_or_else(|| ())?;
        let caps = sample.get_caps().ok_or_else(|| ())?;
        let info = gst_video::VideoInfo::from_caps(caps.as_ref()).ok_or_else(|| ())?;
        let frame =
            gst_video::VideoFrame::from_buffer_readable(buffer, &info).or_else(|_| Err(()))?;

        let buffer = GStreamerBuffer { frame };
        Frame::new(info.width() as i32, info.height() as i32, Arc::new(buffer))
    }

    pub fn setup_video_sink(
        &self,
        pipeline: &gst::Element,
    ) -> Result<gst_app::AppSink, PlayerError> {
        let appsink = gst::ElementFactory::make("appsink", None)
            .ok_or(PlayerError::Backend("appsink creation failed".to_owned()))?
            .dynamic_cast::<gst_app::AppSink>()
            .unwrap();

        let caps = gst::Caps::builder("video/x-raw")
            .field("format", &gst_video::VideoFormat::Bgra.to_string())
            .field("pixel-aspect-ratio", &gst::Fraction::from((1, 1)))
            .build();
        appsink.set_caps(&caps);

        pipeline
            .set_property("video-sink", &appsink)
            .expect("playbin doesn't have expected 'video-sink' property");

        Ok(appsink)
    }
}
