//! `RenderUnix` is a `Render` implementation for Unix-based
//! platforms. It implements an OpenGL mechanism shared by Linux and
//! many of the BSD flavors.
//!
//! Internally it uses GStreamer's *glsinkbin* element as *videosink*
//! wrapping the *appsink* from the Player. And the shared frames are
//! mapped as texture IDs.

use gst::prelude::*;
use gst_gl::prelude::*;
use sm_gst_render;
use sm_gst_render::Render;
use sm_player;
use sm_player::context::{GlApi, GlContext, NativeDisplay, PlayerGLContext};
use sm_player::video::{Buffer, VideoFrame, VideoFrameData};
use sm_player::PlayerError;
use std::sync::{Arc, Mutex};

struct GStreamerBuffer {
    is_external_oes: bool,
    frame: gst_gl::GLVideoFrame<gst_gl::gl_video_frame::Readable>,
}

impl Buffer for GStreamerBuffer {
    fn to_vec(&self) -> Result<VideoFrameData, ()> {
        // packed formats are guaranteed to be in a single plane
        if self.frame.format() == gst_video::VideoFormat::Rgba {
            let tex_id = self.frame.texture_id(0).map_err(|_| ())?;
            Ok(if self.is_external_oes {
                VideoFrameData::OESTexture(tex_id)
            } else {
                VideoFrameData::Texture(tex_id)
            })
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
            GlApi::None => return None,
        };

        let (wrapped_context, display) = match gl_context {
            GlContext::Egl(context) => {
                let display = match display_native {
                    #[cfg(feature = "gl-egl")]
                    NativeDisplay::Egl(display_native) => {
                        unsafe { gstreamer_gl_egl::GLDisplayEGL::with_egl_display(display_native) }
                            .map(|display| display.upcast())
                            .ok()
                    },
                    #[cfg(feature = "gl-wayland")]
                    NativeDisplay::Wayland(display_native) => unsafe {
                        gstreamer_gl_wayland::GLDisplayWayland::with_display(display_native)
                    }
                    .map(|display| display.upcast())
                    .ok(),
                    _ => None,
                };

                RenderUnix::create_wrapped_context(
                    display,
                    context,
                    gst_gl::GLPlatform::EGL,
                    gl_api,
                )
            },
            GlContext::Glx(context) => {
                let display = match display_native {
                    #[cfg(feature = "gl-x11")]
                    NativeDisplay::X11(display_native) => {
                        unsafe { gstreamer_gl_x11::GLDisplayX11::with_display(display_native) }
                            .map(|display| display.upcast())
                            .ok()
                    },
                    _ => None,
                };

                RenderUnix::create_wrapped_context(
                    display,
                    context,
                    gst_gl::GLPlatform::GLX,
                    gl_api,
                )
            },
            GlContext::Unknown => (None, None),
        };

        if let Some(app_context) = wrapped_context {
            let cat = gst::DebugCategory::get("servoplayer").unwrap();
            let _: Result<(), ()> = app_context
                .activate(true)
                .and_then(|_| {
                    app_context.fill_info().or_else(|err| {
                        gst::warning!(
                            cat,
                            "Couldn't fill the wrapped app GL context: {}",
                            err.to_string()
                        );
                        Ok(())
                    })
                })
                .or_else(|_| {
                    gst::warning!(cat, "Couldn't activate the wrapped app GL context");
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

    fn build_frame(&self, sample: gst::Sample) -> Result<VideoFrame, ()> {
        if self.gst_context.lock().unwrap().is_none() && self.gl_upload.lock().unwrap().is_some() {
            *self.gst_context.lock().unwrap() =
                if let Some(glupload) = self.gl_upload.lock().unwrap().as_ref() {
                    Some(glupload.property::<gst_gl::GLContext>("context"))
                } else {
                    None
                };
        }

        let buffer = sample.buffer_owned().ok_or(())?;
        let caps = sample.caps().ok_or(())?;

        let is_external_oes = caps
            .structure(0)
            .and_then(|s| {
                s.get::<&str>("texture-target").ok().and_then(|target| {
                    if target == "external-oes" {
                        Some(s)
                    } else {
                        None
                    }
                })
            })
            .is_some();

        let info = gst_video::VideoInfo::from_caps(caps).map_err(|_| ())?;
        let frame = gst_gl::GLVideoFrame::from_buffer_readable(buffer, &info).map_err(|_| ())?;

        VideoFrame::new(
            info.width() as i32,
            info.height() as i32,
            Arc::new(GStreamerBuffer {
                is_external_oes,
                frame,
            }),
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

        let vsinkbin = gst::ElementFactory::make("glsinkbin")
            .name("servo-media-vsink")
            .build()
            .map_err(|error| {
                PlayerError::Backend(format!("glupload creation failed: {error:?}"))
            })?;

        let caps = gst::Caps::builder("video/x-raw")
            .features([gst_gl::CAPS_FEATURE_MEMORY_GL_MEMORY])
            .field("format", gst_video::VideoFormat::Rgba.to_str())
            .field("texture-target", gst::List::new(["2D", "external-oes"]))
            .build();
        appsink.set_property("caps", caps);

        vsinkbin.set_property("sink", appsink);

        pipeline.set_property("video-sink", &vsinkbin);

        let bus = pipeline.bus().expect("pipeline with no bus");
        let display_ = self.display.clone();
        let context_ = self.app_context.clone();
        bus.set_sync_handler(move |_, msg| {
            match msg.view() {
                gst::MessageView::NeedContext(ctxt) => {
                    if let Some(el) = msg
                        .src()
                        .map(|s| s.clone().downcast::<gst::Element>().unwrap())
                    {
                        let context_type = ctxt.context_type();
                        if context_type == *gst_gl::GL_DISPLAY_CONTEXT_TYPE {
                            let ctxt = gst::Context::new(context_type, true);
                            ctxt.set_gl_display(&display_);
                            el.set_context(&ctxt);
                        } else if context_type == "gst.gl.app_context" {
                            let mut ctxt = gst::Context::new(context_type, true);
                            {
                                let s = ctxt.get_mut().unwrap().structure_mut();
                                s.set_value("context", context_.to_send_value());
                            }
                            el.set_context(&ctxt);
                        }
                    }
                },
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
                    if "glupload" == element.factory().unwrap().name() {
                        break Some(element);
                    }
                },
                Err(gst::IteratorError::Resync) => iter.resync(),
                _ => break None,
            }
        };

        Ok(())
    }
}
