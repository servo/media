use glib::prelude::*;
use gst;
use gst_app;
use gst_video;

use std::sync::Arc;

use servo_media_gstreamer_render::Render;
use servo_media_player::frame::{Buffer, Frame, FrameData};
use servo_media_player::{GlContext, PlayerError};

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
mod platform {
    extern crate servo_media_gstreamer_render_unix;
    pub use self::servo_media_gstreamer_render_unix::RenderUnix as Render;

    use super::*;

    pub fn create_render(context: GlContext, display: usize) -> Option<Render> {
        Render::new(context, display)
    }
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
)))]
mod platform {
    use servo_media_gstreamer_render::Render as RenderTrait;
    use servo_media_player::frame::Frame;
    use servo_media_player::{GlContext, PlayerError};

    pub struct RenderDummy();
    pub type Render = RenderDummy;

    pub fn create_render(_: GlContext, _: usize) -> Option<RenderDummy> {
        None
    }

    impl RenderTrait for RenderDummy {
        fn is_gl(&self) -> bool {
            false
        }

        fn build_frame(&self, _: gst::Buffer, _: gst_video::VideoInfo) -> Result<Frame, ()> {
            Err(())
        }

        fn build_video_sink(&self, _: &gst::Element, _: &gst::Element) -> Result<(), PlayerError> {
            Err(PlayerError::Backend(
                "Not available videosink decorator".to_owned(),
            ))
        }
    }
}

struct GStreamerBuffer {
    frame: gst_video::VideoFrame<gst_video::video_frame::Readable>,
}

impl Buffer for GStreamerBuffer {
    fn to_vec(&self) -> Result<FrameData, ()> {
        let data = self.frame.plane_data(0).ok_or_else(|| ())?;
        Ok(FrameData::Raw(Arc::new(data.to_vec())))
    }
}

pub struct GStreamerRender {
    render: Option<platform::Render>,
}

impl GStreamerRender {
    pub fn new(gl_context: GlContext, display_native: usize) -> Self {
        GStreamerRender {
            render: platform::create_render(gl_context, display_native),
        }
    }

    pub fn is_gl(&self) -> bool {
        if let Some(render) = self.render.as_ref() {
            render.is_gl()
        } else {
            false
        }
    }

    pub fn get_frame_from_sample(&self, sample: &gst::Sample) -> Result<Frame, ()> {
        let buffer = sample.get_buffer().ok_or_else(|| ())?;
        let caps = sample.get_caps().ok_or_else(|| ())?;
        let info = gst_video::VideoInfo::from_caps(caps.as_ref()).ok_or_else(|| ())?;

        if let Some(render) = self.render.as_ref() {
            render.build_frame(buffer, info)
        } else {
            let frame =
                gst_video::VideoFrame::from_buffer_readable(buffer, &info).or_else(|_| Err(()))?;

            Frame::new(
                info.width() as i32,
                info.height() as i32,
                Arc::new(GStreamerBuffer { frame }),
            )
        }
    }

    pub fn setup_video_sink(
        &self,
        pipeline: &gst::Element,
    ) -> Result<gst_app::AppSink, PlayerError> {
        let appsink = gst::ElementFactory::make("appsink", None)
            .ok_or(PlayerError::Backend("appsink creation failed".to_owned()))?;

        if let Some(render) = self.render.as_ref() {
            render.build_video_sink(&appsink, pipeline)?
        } else {
            let caps = gst::Caps::builder("video/x-raw")
                .field("format", &gst_video::VideoFormat::Bgra.to_string())
                .field("pixel-aspect-ratio", &gst::Fraction::from((1, 1)))
                .build();

            appsink
                .set_property("caps", &caps)
                .expect("appsink doesn't have expected 'caps' property");

            pipeline
                .set_property("video-sink", &appsink)
                .expect("playbin doesn't have expected 'video-sink' property");
        };

        let appsink = appsink.dynamic_cast::<gst_app::AppSink>().unwrap();
        Ok(appsink)
    }
}
