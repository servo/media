extern crate servo_media;
extern crate servo_media_auto;

use servo_media::ServoMedia;
use std::sync::Arc;

fn run_example(servo_media: Arc<ServoMedia>) {
    let source = servo_media
        .create_mse_source()
        .unwrap();
    source.on_source_open(Box::new(move |s| {
        let buffer = s.add_source_buffer("video/mp4; codecs=\"avc1.42E01E, mp4a.40.2\"");
        let append_data = include_bytes!("resources/mov_bbb.mp4");
        buffer.append_buffer(append_data.to_vec());
    }));
}

fn main() {
    ServoMedia::init::<servo_media_auto::Backend>();
    let servo_media = ServoMedia::get();
    run_example(servo_media);
}
