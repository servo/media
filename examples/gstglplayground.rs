extern crate gstreamer as gst;
#[cfg(target_os = "macos")]
extern crate servo_media_gstgl_macos_bindings;

#[cfg(target_os = "macos")]
use servo_media_gstgl_macos_bindings::*;

fn main() {
    gst::init().unwrap();

    #[cfg(target_os = "macos")]
    println!("{:?}", GLDisplayCocoa::new());
}
