// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use servo_media::player::context::*;
use std::mem;
use std::sync::Mutex;

pub struct PlayerContextGlutin {
    gl_context: GlContext,
    native_display: NativeDisplay,
    gl_api: GlApi,
}

lazy_static! {
    static ref CHOOSE_PIXEL_FORMAT_MUTEX: Mutex<()> = Mutex::new(());
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
                use glutin::os::unix::WindowExt;

                let gl_context = {
                    use glutin::os::unix::RawHandle;

                    match raw_handle {
                        RawHandle::Egl(egl_context) => GlContext::Egl(egl_context as usize),
                        RawHandle::Glx(glx_context) => GlContext::Glx(glx_context as usize),
                    }
                };

                let native_display =
                    if let Some(display) = windowed_context.window().get_wayland_display() {
                        NativeDisplay::Wayland(display as usize)
                    } else if let Some(display) = windowed_context.window().get_xlib_display() {
                        NativeDisplay::X11(display as usize)
                    } else if let Some(display) =
                        unsafe { windowed_context.context().get_egl_display() }
                    {
                        NativeDisplay::Egl(display as usize)
                    } else {
                        NativeDisplay::Unknown
                    };

                let gl_api = match api {
                    glutin::Api::OpenGl => GlApi::OpenGL3,
                    glutin::Api::OpenGlEs => GlApi::Gles2,
                    _ => GlApi::None,
                };

                (gl_context, native_display, gl_api)
            }

            #[cfg(target_os = "macos")]
            {
                #[allow(non_upper_case_globals)]
                const kCGLOGLPVersion_3_2_Core: cgl::CGLPixelFormatAttribute = 0x3200;

                // CGLChoosePixelFormat fails if multiple threads try to open a display connection
                // simultaneously. The following error is returned by CGLChoosePixelFormat:
                // kCGLBadConnection - Invalid connection to Core Graphics.
                // We use a static mutex guard to fix this issue
                let _guard = CHOOSE_PIXEL_FORMAT_MUTEX.lock().unwrap();

                let mut attributes = [cgl::kCGLPFAOpenGLProfile, kCGLOGLPVersion_3_2_Core, 0];

                let mut pixel_format = mem::MaybeUninit::uninit();
                let mut pix_count = 0;

                let pixel_format = unsafe {
                    if cgl::CGLChoosePixelFormat(
                        attributes.as_mut_ptr(),
                        pixel_format.as_mut_ptr(),
                        &mut pix_count,
                    ) != 0
                    {
                        panic!();
                    }

                    if pix_count == 0 {
                        panic!();
                    }
                    pixel_format.assume_init()
                };

                let mut native = mem::MaybeUninit::uninit();

                let native = unsafe {
                    // XXX: if a new context is created, not a shared one, the same error when drawing
                    if cgl::CGLCreateContext(pixel_format, raw_handle as _, native.as_mut_ptr())
                        != 0
                    {
                        // we face the same problem
                        // https://github.com/servo/rust-offscreen-rendering-context/issues/82
                        panic!();
                    }
                    native.assume_init()
                };

                unsafe {
                    if cgl::CGLDestroyPixelFormat(pixel_format) != 0 {
                        eprintln!("CGLDestroyPixelformat errored");
                    }
                }

                let gl_context = GlContext::Cgl(native as usize);
                let gl_api = match api {
                    glutin::Api::OpenGl => GlApi::OpenGL3,
                    _ => GlApi::None,
                };

                (gl_context, NativeDisplay::Unknown, gl_api)
            }

            #[cfg(not(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd",
                target_os = "macos",
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

// XXX(victor): ensure the lifetime
#[cfg(target_os = "macos")]
impl Drop for PlayerContextGlutin {
    fn drop(&mut self) {
	let context = match self.gl_context {
	    GlContext::Cgl(ctxt) => ctxt as cgl::CGLContextObj,
	    _ => return
	};
	unsafe {
	    if cgl::CGLGetCurrentContext() == context {
		cgl::CGLSetCurrentContext(0 as cgl::CGLContextObj);
	    }
	    cgl::CGLDestroyContext(context);
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
