// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use servo_media::player::context::*;

pub struct PlayerContextGlutin {
    gl_context: GlContext,
    native_display: NativeDisplay,
    gl_api: GlApi,
}

#[allow(unused_variables)]
impl PlayerContextGlutin {
    pub fn new(
        use_gl: bool,
        windowed_context: &glutin::WindowedContext<glutin::PossiblyCurrent>,
    ) -> Self {
        if !use_gl {
            return Self {
                gl_context: GlContext::Unknown,
                native_display: NativeDisplay::Unknown,
                gl_api: GlApi::None,
            };
        }

        let (gl_context, native_display, gl_api) = {
            use glutin::os::ContextTraitExt;

            let context = windowed_context.context();
            let raw_handle = unsafe { context.raw_handle() };
            let api = windowed_context.get_api();

            #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            ))]
            {
                let gl_context = {
                    use glutin::os::unix::RawHandle;

                    match raw_handle {
                        RawHandle::Egl(egl_context) => GlContext::Egl(egl_context as usize),
                        RawHandle::Glx(glx_context) => GlContext::Glx(glx_context as usize),
                    }
                };
                let native_display = if let Some(display) =
                    unsafe { windowed_context.context().get_egl_display() }
                {
                    NativeDisplay::Egl(display as usize)
                } else {
                    use glutin::os::unix::WindowExt;

                    if let Some(display) = windowed_context.window().get_wayland_display() {
                        NativeDisplay::Wayland(display as usize)
                    } else if let Some(display) = windowed_context.window().get_xlib_display() {
                        NativeDisplay::X11(display as usize)
                    } else {
                        NativeDisplay::Unknown
                    }
                };

                let gl_api = match api {
                    glutin::Api::OpenGl => GlApi::OpenGL3,
                    glutin::Api::OpenGlEs => GlApi::Gles2,
                    _ => GlApi::None,
                };

                (gl_context, native_display, gl_api)
            }

            #[cfg(not(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            )))]
            {
                println!("GL rendering unavailable for this platform");
                (GlContext::Unknown, NativeDisplay::Unknown, GlApi::None)
            }
        };

        Self {
            gl_context,
            native_display,
            gl_api,
        }
    }
}

impl PlayerGLContext for PlayerContextGlutin {
    fn get_gl_context(&self) -> GlContext {
        self.gl_context.clone()
    }

    fn get_native_display(&self) -> NativeDisplay {
        self.native_display.clone()
    }

    fn get_gl_api(&self) -> GlApi {
        self.gl_api.clone()
    }
}
