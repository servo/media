/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#[macro_use]
extern crate glib;
extern crate glib_sys;
extern crate gstreamer as gst;
extern crate gstreamer_sys as gst_sys;
extern crate gstreamer_gl as gst_gl;
extern crate servo_media_gst_gl_macos_bindings_sys as gst_gl_macos_sys;

use glib::translate::*;

macro_rules! assert_initialized_main_thread {
    () => {
        if unsafe { ::gst_sys::gst_is_initialized() } != ::glib_sys::GTRUE {
            panic!("GStreamer has not been initialized. Call `gst::init` first.");
        }
    };
}

glib_wrapper! {
    pub struct GLDisplayCocoa(Object<gst_gl_macos_sys::GstGLDisplayCocoa, gst_gl_macos_sys::GstGLDisplayCocoaClass, GLDisplayCocoaClass>) @extends gst_gl::GLDisplay, gst::Object;

    match fn {
        get_type => || gst_gl_macos_sys::gst_gl_display_cocoa_get_type(),
    }
}

impl GLDisplayCocoa {
    pub fn new() -> GLDisplayCocoa {
        assert_initialized_main_thread!();
        unsafe { from_glib_full(gst_gl_macos_sys::gst_gl_display_cocoa_new()) }
    }
}

impl Default for GLDisplayCocoa {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl Send for GLDisplayCocoa {}
unsafe impl Sync for GLDisplayCocoa {}
