/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#[macro_use]
extern crate glib;
extern crate glib_sys;
extern crate gstreamer as gst;
extern crate gstreamer_gl as gst_gl;
extern crate gstreamer_gl_sys as gst_gl_sys;
extern crate gstreamer_sys as gst_sys;

mod api;
mod sys;

pub use api::*;
