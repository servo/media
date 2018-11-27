extern crate servo_media;

use servo_media::audio::node::{AudioNodeInit, AudioNodeMessage, AudioScheduledSourceNodeMessage};
use servo_media::audio::constant_source::ConstantSourceNodeOptions;

use servo_media::ServoMedia;
use std::sync::Arc;
use std::{thread, time};

fn run_example(servo_media: Arc<ServoMedia>) {
    let context = servo_media.create_audio_context(Default::default());
    let dest = context.dest_node();
    let mut options = ConstantSourceNodeOptions::default();
    options.offset = 15.;
    let cs = context.create_node(
        AudioNodeInit::ConstantSourceNode(options.clone()),
        Default::default(),
    );


    context.connect_ports(cs.output(0), dest.input(0));
    let _ = context.resume();
    context.message_node(
        cs,
        AudioNodeMessage::AudioScheduledSourceNode(AudioScheduledSourceNodeMessage::Start(0.)),
    );

    thread::sleep(time::Duration::from_millis(3000));
    let _ = context.close();
    thread::sleep(time::Duration::from_millis(3000));


}

fn main() {
    if let Ok(servo_media) = ServoMedia::get() {
        run_example(servo_media);
    } else {
        unreachable!();
    }
}
