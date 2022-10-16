extern crate servo_media;
extern crate servo_media_auto;

use servo_media::streams::MediaSource;
use servo_media::ServoMedia;
use std::sync::Arc;
use std::{thread, time};

fn run_example(servo_media: Arc<ServoMedia>) {
    if let Some(stream) =
        servo_media.create_videoinput_stream(Default::default(), MediaSource::Device)
    {
        let mut output = servo_media.create_stream_output();
        output.add_stream(&stream);
        thread::sleep(time::Duration::from_millis(6000));
    } else {
        print!("No video input elements available");
    }
}

fn main() {
    ServoMedia::init::<servo_media_auto::Backend>();
    if let Ok(servo_media) = ServoMedia::get() {
        run_example(servo_media);
    } else {
        unreachable!();
    }
}
