//! `RenderMacOS` is a `Render` implementation for MacOS.

#![cfg(target_os = "macos")]

extern crate gstreamer_gl as gst_gl;

pub struct RenderMacOS {
    display: gst_gl::GLDisplay,
}

impl RenderMacOS {}
