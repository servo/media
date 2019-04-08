#![cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]

extern crate glib;
extern crate gstreamer as gst;
extern crate gstreamer_gl as gst_gl;
extern crate gstreamer_video as gst_video;

extern crate servo_media_gstreamer_render as sm_gst_render;
extern crate servo_media_player as sm_player;

use gst::prelude::*;
use gst_gl::prelude::*;
use sm_gst_render::Render;
use sm_player::frame::{Buffer, Frame, FrameData};
use sm_player::{GlContext, PlayerError};
use std::sync::{Arc, Mutex};

struct GStreamerBuffer {
    frame: gst_video::VideoFrame<gst_video::video_frame::Readable>,
}

impl Buffer for GStreamerBuffer {
    fn to_vec(&self) -> Result<FrameData, ()> {
        // packed formats are guaranteed to be in a single plane
        if self.frame.format() == gst_video::VideoFormat::Bgrx {
            let tex_id = self.frame.get_texture_id(0).ok_or_else(|| ())?;
            Ok(FrameData::Texture(tex_id))
        } else {
            Err(())
        }
    }
}

pub struct RenderUnix {
    display: gst_gl::GLDisplay,
    app_context: gst_gl::GLContext,
    gst_context: Arc<Mutex<Option<gst_gl::GLContext>>>,
    gl_upload: Arc<Mutex<Option<gst::Element>>>,
}

impl RenderUnix {
    pub fn new(gl_context: GlContext, display_native: usize) -> Option<RenderUnix> {
        match gl_context {
            GlContext::Egl(context) => {
                let display = unsafe { gst_gl::GLDisplayEGL::new_with_egl_display(display_native) };
                if let Some(display) = display {
                    let context = unsafe {
                        gst_gl::GLContext::new_wrapped(
                            &display,
                            context,
                            gst_gl::GLPlatform::EGL,
                            gst_gl::GLAPI::ANY,
                        )
                    };

                    if let Some(context) = context {
                        if !(context.activate(true).is_ok() && context.fill_info().is_ok()) {
                            println!("Couldn't fill the wrapped app GL context")
                        }
                        return Some(RenderUnix {
                            display: display.upcast(),
                            app_context: context,
                            gst_context: Arc::new(Mutex::new(None)),
                            gl_upload: Arc::new(Mutex::new(None)),
                        });
                    }
                }

                None
            }
            _ => None,
        }
    }
}

impl Render for RenderUnix {
    fn is_gl(&self) -> bool {
        true
    }

    fn build_frame(&self, buffer: gst::Buffer, info: gst_video::VideoInfo) -> Result<Frame, ()> {
        if self.gst_context.lock().unwrap().is_none() && self.gl_upload.lock().unwrap().is_some() {
            *self.gst_context.lock().unwrap() =
                if let Some(glupload) = self.gl_upload.lock().unwrap().as_ref() {
                    glupload
                        .get_property("context")
                        .or_else(|_| Err(()))?
                        .get::<gst_gl::GLContext>()
                } else {
                    None
                };
        }

        let frame =
            gst_video::VideoFrame::from_buffer_readable_gl(buffer, &info).or_else(|_| Err(()))?;

        Frame::new(
            info.width() as i32,
            info.height() as i32,
            Arc::new(GStreamerBuffer { frame }),
        )
    }

    fn build_video_sink(
        &self,
        appsink: &gst::Element,
        pipeline: &gst::Element,
    ) -> Result<(), PlayerError> {
        if self.gl_upload.lock().unwrap().is_some() {
            return Err(PlayerError::Backend(
                "render unix already setup the video sink".to_owned(),
            ));
        }

        let vsinkbin = gst::Bin::new("servo-media-video-sink");

        let glupload = gst::ElementFactory::make("glupload", "servo-media-upload")
            .ok_or(PlayerError::Backend("glupload creation failed".to_owned()))?;
        let glconvert = gst::ElementFactory::make("glcolorconvert", None).ok_or(
            PlayerError::Backend("glcolorconvert creation failed".to_owned()),
        )?;

        let caps = gst::Caps::builder("video/x-raw")
            .features(&[&gst_gl::CAPS_FEATURE_MEMORY_GL_MEMORY])
            .field("format", &gst_video::VideoFormat::Bgrx.to_string())
            .field("texture-target", &"2D")
            .build();
        appsink
            .set_property("caps", &caps)
            .or_else(|err| Err(PlayerError::Backend(err.to_string())))?;

        vsinkbin
            .add_many(&[&glupload, &glconvert, appsink])
            .expect("Could not add elements into video sink bin");

        gst::Element::link_many(&[&glupload, &glconvert, appsink])
            .expect("Could not link elements in video sink bin");

        let pad = glupload
            .get_static_pad("sink")
            .expect("glupload doesn't have sink pad");
        let ghost_pad = gst::GhostPad::new("sink", &pad).expect("Could not create ghost pad");
        vsinkbin
            .add_pad(&ghost_pad)
            .expect("Could not add gohst pad to video sink bin");

        pipeline
            .set_property("video-sink", &vsinkbin)
            .expect("playbin doesn't have expected 'video-sink' property");

        let bus = pipeline.get_bus().expect("pipeline with no bus");
        let display_ = self.display.clone();
        let context_ = self.app_context.clone();
        bus.set_sync_handler(move |_, msg| {
            match msg.view() {
                gst::MessageView::NeedContext(ctxt) => {
                    if let Some(el) = msg.get_src().map(|s| s.downcast::<gst::Element>().unwrap()) {
                        let context_type = ctxt.get_context_type();
                        if context_type == *gst_gl::GL_DISPLAY_CONTEXT_TYPE {
                            let ctxt = gst::Context::new(context_type, true);
                            ctxt.set_gl_display(&display_);
                            el.set_context(&ctxt);
                        } else if context_type == "gst.gl.app_context" {
                            let mut ctxt = gst::Context::new(context_type, true);
                            {
                                let mut s = ctxt.get_mut().unwrap().get_mut_structure();
                                s.set_value("context", context_.to_send_value());
                            }
                            el.set_context(&ctxt);
                        }
                    }
                }
                _ => (),
            }

            gst::BusSyncReply::Pass
        });

        *self.gl_upload.lock().unwrap() = Some(glupload);

        Ok(())
    }
}
