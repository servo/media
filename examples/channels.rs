extern crate servo_media;

use servo_media::audio::channel_node::ChannelNodeOptions;
use servo_media::audio::gain_node::GainNodeOptions;
use servo_media::audio::node::{AudioNodeMessage, AudioNodeType, AudioScheduledSourceNodeMessage};
use servo_media::ServoMedia;
use std::sync::Arc;
use std::{thread, time};

fn run_example(servo_media: Arc<ServoMedia>) {
    let context = servo_media.create_audio_context(Default::default());
    let mut options = Default::default();
    let osc = context.create_node(AudioNodeType::OscillatorNode(options));
    options.freq = 400.;
    let osc2 = context.create_node(AudioNodeType::OscillatorNode(options));
    let mut options = GainNodeOptions::default();
    options.gain = 0.5;
    let gain = context.create_node(AudioNodeType::GainNode(options));
    let options = ChannelNodeOptions { channels: 2 };
    let merger = context.create_node(AudioNodeType::ChannelMergerNode(options));

    let dest = context.dest_node();
    context.connect_ports(osc.output(0), gain.input(0));
    context.connect_ports(gain.output(0), merger.input(0));
    context.connect_ports(osc2.output(0), merger.input(1));
    context.connect_ports(merger.output(0), dest.input(0));
    context.message_node(
        osc,
        AudioNodeMessage::AudioScheduledSourceNode(AudioScheduledSourceNodeMessage::Start(0.)),
        );
    context.message_node(
        osc2,
        AudioNodeMessage::AudioScheduledSourceNode(AudioScheduledSourceNodeMessage::Start(0.)),
        );
    let _ = context.resume();

    thread::sleep(time::Duration::from_millis(5000));
    let _ = context.close();

}

fn main() {
    if let Ok(servo_media) = ServoMedia::get() {
        run_example(servo_media);
    } else {
        unreachable!();
    }
}
