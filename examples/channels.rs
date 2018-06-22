extern crate servo_media;

use servo_media::audio::channel_node::ChannelNodeOptions;
use servo_media::audio::gain_node::GainNodeOptions;
use servo_media::audio::node::{AudioNodeMessage, AudioNodeType};
use servo_media::audio::oscillator_node::OscillatorNodeMessage;
use servo_media::ServoMedia;
use std::{thread, time};

fn main() {
    if let Ok(servo_media) = ServoMedia::get() {
        let mut graph = servo_media.create_audio_graph();
        let mut options = Default::default();
        let osc = graph.create_node(AudioNodeType::OscillatorNode(options));
        options.freq = 400.;
        let osc2 = graph.create_node(AudioNodeType::OscillatorNode(options));
        let mut options = GainNodeOptions::default();
        options.gain = 0.5;
        let gain = graph.create_node(AudioNodeType::GainNode(options));
        let options = ChannelNodeOptions { channels: 2 };
        let merger = graph.create_node(AudioNodeType::ChannelMergerNode(options));

        let dest = graph.dest_node();
        graph.connect_ports(osc.output(0), gain.input(0));
        graph.connect_ports(gain.output(0), merger.input(0));
        graph.connect_ports(osc2.output(0), merger.input(1));
        graph.connect_ports(merger.output(0), dest.input(0));
        graph.message_node(
            osc,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::Start(0.)),
        );
        graph.message_node(
            osc2,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::Start(0.)),
        );
        graph.resume();


        thread::sleep(time::Duration::from_millis(5000));
        graph.close();
    } else {
        unreachable!();
    }
}
