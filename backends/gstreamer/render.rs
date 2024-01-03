use glib::prelude::*;

use std::sync::Arc;

use servo_media_gstreamer_render::Render;
use servo_media_player::context::PlayerGLContext;
use servo_media_player::video::{Buffer, VideoFrame, VideoFrameData};
use servo_media_player::PlayerError;

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

    pub fn create_render(gl_context: Box<dyn PlayerGLContext>) -> Option<Render> {
        Render::new(gl_context)
    }
}

#[cfg(target_os = "android")]
mod platform {
    extern crate servo_media_gstreamer_render_android;
    pub use self::servo_media_gstreamer_render_android::RenderAndroid as Render;

    use super::*;

    pub fn create_render(gl_context: Box<dyn PlayerGLContext>) -> Option<Render> {
        Render::new(gl_context)
    }
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "android",
)))]
mod platform {
    use servo_media_gstreamer_render::Render as RenderTrait;
    use servo_media_player::context::PlayerGLContext;
    use servo_media_player::video::VideoFrame;
    use servo_media_player::PlayerError;

    pub struct RenderDummy();
    pub type Render = RenderDummy;

    pub fn create_render(_: Box<dyn PlayerGLContext>) -> Option<RenderDummy> {
        None
    }

    impl RenderTrait for RenderDummy {
        fn is_gl(&self) -> bool {
            false
        }

        fn build_frame(&self, _: gst::Sample) -> Result<VideoFrame, ()> {
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
    fn to_vec(&self) -> Result<VideoFrameData, ()> {
        let data = self.frame.plane_data(0).map_err(|_| ())?;
        Ok(VideoFrameData::Raw(Arc::new(data.to_vec())))
    }
}

pub struct GStreamerRender {
    render: Option<platform::Render>,
}

impl GStreamerRender {
    pub fn new(gl_context: Box<dyn PlayerGLContext>) -> Self {
        GStreamerRender {
            render: platform::create_render(gl_context),
        }
    }

    pub fn is_gl(&self) -> bool {
        if let Some(render) = self.render.as_ref() {
            render.is_gl()
        } else {
            false
        }
    }

    pub fn get_frame_from_sample(&self, sample: gst::Sample) -> Result<VideoFrame, ()> {
        if let Some(render) = self.render.as_ref() {
            render.build_frame(sample)
        } else {
            let buffer = sample.buffer_owned().ok_or(())?;
            let caps = sample.caps().ok_or(())?;
            let info = gst_video::VideoInfo::from_caps(caps).map_err(|_| ())?;

            let frame =
                gst_video::VideoFrame::from_buffer_readable(buffer, &info).map_err(|_| ())?;

            VideoFrame::new(
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
        let appsink = gst::ElementFactory::make("appsink")
            .build()
            .map_err(|error| PlayerError::Backend(format!("appsink creation failed: {error:?}")))?
            .downcast::<gst_app::AppSink>()
            .unwrap();

        if let Some(render) = self.render.as_ref() {
            render.build_video_sink(appsink.upcast_ref::<gst::Element>(), pipeline)?
        } else {
            let caps = gst::Caps::builder("video/x-raw")
                .field("format", gst_video::VideoFormat::Bgra.to_str())
                .field("pixel-aspect-ratio", gst::Fraction::from((1, 1)))
                .build();

            appsink.set_caps(Some(&caps));
            pipeline.set_property("video-sink", &appsink);
        };

        Ok(appsink)
    }
}
