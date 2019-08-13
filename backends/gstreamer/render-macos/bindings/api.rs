/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::sys;

use glib::translate::*;
use gst;
use gst_gl::GLDisplay;

macro_rules! assert_initialized_main_thread {
    () => {
        if unsafe { ::gst_sys::gst_is_initialized() } != ::glib_sys::GTRUE {
            panic!("GStreamer has not been initialized. Call `gst::init` first.");
        }
    };
}

glib_wrapper! {
    pub struct GLDisplayCocoa(Object<sys::GstGLDisplayCocoa, sys::GstGLDisplayCocoaClass, GLDisplayCocoaClass>) @extends GLDisplay, gst::Object;

    match fn {
        get_type => || sys::gst_gl_display_cocoa_get_type(),
    }
}

impl GLDisplayCocoa {
    pub fn new() -> GLDisplayCocoa {
        assert_initialized_main_thread!();
        unsafe { from_glib_full(sys::gst_gl_display_cocoa_new()) }
    }
}

impl Default for GLDisplayCocoa {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl Send for GLDisplayCocoa {}
unsafe impl Sync for GLDisplayCocoa {}
