/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#![allow(non_camel_case_types, non_upper_case_globals, non_snake_case)]

extern crate glib_sys as glib;
extern crate gstreamer_gl_sys as gst_gl;

use glib::GType;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct GstGLDisplayCocoaClass {
    pub object_class: gst_gl::GstGLDisplayClass,
}

impl ::std::fmt::Debug for GstGLDisplayCocoaClass {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        f.debug_struct(&format!("GstGLDisplayCocoaClass @ {:?}", self as *const _))
            .field("object_class", &self.object_class)
            .finish()
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct GstGLDisplayCocoa {
    pub parent: gst_gl::GstGLDisplay,
}

impl ::std::fmt::Debug for GstGLDisplayCocoa {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        f.debug_struct(&format!("GstGLDisplayCocoa @ {:?}", self as *const _))
            .field("parent", &self.parent)
            .finish()
    }
}

extern "C" {
    //=========================================================================
    // GstGLDisplayCocoa
    //=========================================================================
    pub fn gst_gl_display_cocoa_get_type() -> GType;
    pub fn gst_gl_display_cocoa_new() -> *mut GstGLDisplayCocoa;
}
