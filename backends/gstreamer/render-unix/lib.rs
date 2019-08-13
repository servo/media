//! `RenderUnix` is a `Render` implementation for Unix-based
//! platforms. It implements an OpenGL mechanism shared by Linux and
//! many of the BSD flavors.
//!
//! Internally it uses GStreamer's *glsinkbin* element as *videosink*
//! wrapping the *appsink* from the Player. And the shared frames are
//! mapped as texture IDs.

#![cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]

#[macro_use]
extern crate gstreamer as gst;
extern crate gstreamer_gl as gst_gl;
extern crate gstreamer_video as gst_video;

extern crate servo_media_gstreamer_render as sm_gst_render;
extern crate servo_media_player as sm_player;

use gst::prelude::*;
use gst_gl::prelude::*;
use sm_gst_render::Render;
use sm_player::context::{GlApi, GlContext, NativeDisplay, PlayerGLContext};
use sm_player::frame::{Buffer, Frame, FrameData};
use sm_player::PlayerError;
use std::sync::{Arc, Mutex};

struct GStreamerBuffer {
    frame: gst_video::VideoFrame<gst_video::video_frame::Readable>,
}

impl Buffer for GStreamerBuffer {
    fn to_vec(&self) -> Result<FrameData, ()> {
        // packed formats are guaranteed to be in a single plane
        if self.frame.format() == gst_video::VideoFormat::Rgba {
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
    /// Tries to create a new intance of the `RenderUnix`
    ///
    /// # Arguments
    ///
    /// * `context` - is the PlayerContext trait object from
    /// application.
    pub fn new(app_gl_context: Box<dyn PlayerGLContext>) -> Option<RenderUnix> {
        // Check that we actually have the elements that we
        // need to make this work.
        if gst::ElementFactory::find("glsinkbin").is_none() {
            return None;
        }

        let display_native = app_gl_context.get_native_display();
        let gl_context = app_gl_context.get_gl_context();
        let gl_api = match app_gl_context.get_gl_api() {
            GlApi::OpenGL => gst_gl::GLAPI::OPENGL,
            GlApi::OpenGL3 => gst_gl::GLAPI::OPENGL3,
            GlApi::Gles1 => gst_gl::GLAPI::GLES1,
            GlApi::Gles2 => gst_gl::GLAPI::GLES2,
            GlApi::None => gst_gl::GLAPI::NONE,
        };

        let (wrapped_context, display) = match gl_context {
            GlContext::Egl(context) => {
                let display = match display_native {
                    #[cfg(feature = "gl-egl")]
                    NativeDisplay::Egl(display_native) => {
                        unsafe { gst_gl::GLDisplayEGL::new_with_egl_display(display_native) }
                            .and_then(|display| Some(display.upcast()))
                    }
                    #[cfg(feature = "gl-wayland")]
                    NativeDisplay::Wayland(display_native) => {
                        unsafe { gst_gl::GLDisplayWayland::new_with_display(display_native) }
                            .and_then(|display| Some(display.upcast()))
                    }
                    _ => None,
                };

                RenderUnix::create_wrapped_context(
                    display,
                    context,
                    gst_gl::GLPlatform::EGL,
                    gl_api,
                )
            }
            GlContext::Glx(context) => {
                let display = match display_native {
                    #[cfg(feature = "gl-x11")]
                    NativeDisplay::X11(display_native) => {
                        unsafe { gst_gl::GLDisplayX11::new_with_display(display_native) }
                            .and_then(|display| Some(display.upcast()))
                    }
                    _ => None,
                };

                RenderUnix::create_wrapped_context(
                    display,
                    context,
                    gst_gl::GLPlatform::GLX,
                    gl_api,
                )
            }
            GlContext::Unknown => (None, None),
        };

        if let Some(app_context) = wrapped_context {
            let cat = gst::DebugCategory::get("servoplayer").unwrap();
            let _: Result<(), ()> = app_context
                .activate(true)
                .and_then(|_| {
                    app_context.fill_info().or_else(|err| {
                        gst_warning!(
                            cat,
                            "Couldn't fill the wrapped app GL context: {}",
                            err.to_string()
                        );
                        Ok(())
                    })
                })
                .or_else(|_| {
                    gst_warning!(cat, "Couldn't activate the wrapped app GL context");
                    Ok(())
                });
            Some(RenderUnix {
                display: display.unwrap(),
                app_context,
                gst_context: Arc::new(Mutex::new(None)),
                gl_upload: Arc::new(Mutex::new(None)),
            })
        } else {
            None
        }
    }

    fn create_wrapped_context(
        display: Option<gst_gl::GLDisplay>,
        handle: usize,
        platform: gst_gl::GLPlatform,
        api: gst_gl::GLAPI,
    ) -> (Option<gst_gl::GLContext>, Option<gst_gl::GLDisplay>) {
        if let Some(display) = display {
            let wrapped_context =
                unsafe { gst_gl::GLContext::new_wrapped(&display, handle, platform, api) };
            (wrapped_context, Some(display))
        } else {
            (None, None)
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

        let vsinkbin = gst::ElementFactory::make("glsinkbin", Some("servo-media-vsink"))
            .ok_or(PlayerError::Backend("glupload creation failed".to_owned()))?;

        let caps = gst::Caps::builder("video/x-raw")
            .features(&[&gst_gl::CAPS_FEATURE_MEMORY_GL_MEMORY])
            .field("format", &gst_video::VideoFormat::Rgba.to_string())
            .field("texture-target", &"2D")
            .build();
        appsink
            .set_property("caps", &caps)
            .expect("appsink doesn't have expected 'caps' property");

        vsinkbin
            .set_property("sink", &appsink)
            .expect("glsinkbin doesn't have expected 'sink' property");

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
                                let s = ctxt.get_mut().unwrap().get_mut_structure();
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

        let mut iter = vsinkbin
            .dynamic_cast::<gst::Bin>()
            .unwrap()
            .iterate_elements();
        *self.gl_upload.lock().unwrap() = loop {
            match iter.next() {
                Ok(Some(element)) => {
                    if "glupload" == element.get_factory().unwrap().get_name() {
                        break Some(element);
                    }
                }
                Err(gst::IteratorError::Resync) => iter.resync(),
                _ => break None,
            }
        };

        Ok(())
    }
}
