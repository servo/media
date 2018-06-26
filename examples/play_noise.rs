extern crate rand;
extern crate servo_media;

use servo_media::audio::buffer_source_node::AudioBufferSourceNodeMessage;
use servo_media::audio::node::{AudioNodeMessage, AudioNodeType, AudioScheduledSourceNodeMessage};
use servo_media::ServoMedia;
use std::sync::Arc;
use std::{thread, time};

fn run_example(servo_media: Arc<ServoMedia>) {
    let context = servo_media.create_audio_context(Default::default());
    let buffer_source = context.create_node(AudioNodeType::AudioBufferSourceNode(Default::default()));
    let dest = context.dest_node();
    context.connect_ports(buffer_source.output(0), dest.input(0));
    let mut buffer = Vec::with_capacity(4096);
    for _ in 0..4096 {
        buffer.push(rand::random::<f32>());
    }
    context.message_node(
        buffer_source,
        AudioNodeMessage::AudioScheduledSourceNode(AudioScheduledSourceNodeMessage::Start(0.)),
    );
    context.message_node(
        buffer_source,
        AudioNodeMessage::AudioBufferSourceNode(AudioBufferSourceNodeMessage::SetBuffer(buffer)),
    );
    let _ = context.resume();
    thread::sleep(time::Duration::from_millis(5000));
    let _ = context.close();
}

fn main() {
    if let Ok(servo_media) = ServoMedia::get() {
        run_example(servo_media);
    } else {
        unreachable!()
    }
}
