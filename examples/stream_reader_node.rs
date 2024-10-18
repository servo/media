extern crate servo_media;
extern crate servo_media_auto;

use servo_media::audio::node::AudioNodeInit;
use servo_media::{ClientContextId, ServoMedia};
use std::sync::Arc;
use std::{thread, time};

fn run_example(servo_media: Arc<ServoMedia>) {
    let context =
        servo_media.create_audio_context(&ClientContextId::build(1, 1), Default::default());
    let input = servo_media.create_audiostream();
    let context = context.unwrap();
    let context = context.lock().unwrap();
    let dest = context.dest_node();
    let osc1 = context.create_node(
        AudioNodeInit::MediaStreamSourceNode(input),
        Default::default(),
    );
    context.connect_ports(osc1.output(0), dest.input(0));
    let _ = context.resume();

    thread::sleep(time::Duration::from_millis(6000));
    let _ = context.close();
}

fn main() {
    ServoMedia::init::<servo_media_auto::Backend>();
    if let Ok(servo_media) = ServoMedia::get() {
        run_example(servo_media);
    } else {
        unreachable!();
    }
}
