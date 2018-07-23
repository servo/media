extern crate servo_media;

use servo_media::audio::block::FRAMES_PER_BLOCK_USIZE;
use servo_media::audio::context::{AudioContextOptions, OfflineAudioContextOptions};
use servo_media::audio::node::{AudioNodeInit, AudioNodeMessage, AudioScheduledSourceNodeMessage};
use servo_media::ServoMedia;
use std::{thread, time};
use std::sync::Arc;

fn run_example(servo_media: Arc<ServoMedia>) {
    let mut options = <OfflineAudioContextOptions>::default();
    options.channels = 2;
    options.length = 10 * FRAMES_PER_BLOCK_USIZE;
    let options = AudioContextOptions::OfflineAudioContext(options);
    let context = servo_media.create_audio_context(options);
    let osc = context.create_node(AudioNodeInit::OscillatorNode(Default::default()));
    let dest = context.dest_node();
    context.connect_ports(osc.output(0), dest.input(0));
    context.message_node(
        osc,
        AudioNodeMessage::AudioScheduledSourceNode(AudioScheduledSourceNodeMessage::Start(0.)),
    );
    let _ = context.resume();
    thread::sleep(time::Duration::from_millis(3000));
    let _ = context.close();
}

fn main() {
    if let Ok(servo_media) = ServoMedia::get() {
        run_example(servo_media);
    } else {
        unreachable!()
    }
}
