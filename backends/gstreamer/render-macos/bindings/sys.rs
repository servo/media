/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use gst_gl_sys::{GstGLDisplay, GstGLDisplayClass};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct GstGLDisplayCocoaClass {
    pub object_class: GstGLDisplayClass,
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
    pub parent: GstGLDisplay,
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
    pub fn gst_gl_display_cocoa_get_type() -> glib_sys::GType;
    pub fn gst_gl_display_cocoa_new() -> *mut GstGLDisplayCocoa;
}
