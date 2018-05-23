extern crate servo_media;

use servo_media::audio::node::{AudioNodeType, AudioNodeMessage};
use servo_media::*;
use servo_media::audio::gain_node::GainNodeOptions;
use std::{thread, time};

fn main() {
    if let Ok(servo_media) = ServoMedia::get() {
        let mut graph = servo_media.create_audio_graph().unwrap();
        graph.create_node(AudioNodeType::OscillatorNode(Default::default()));
        let mut options = GainNodeOptions::default();
        options.gain = 0.5;
        graph.create_node(AudioNodeType::GainNode(options));
        graph.resume_processing();
        thread::sleep(time::Duration::from_millis(2000));
        graph.message_node(0, AudioNodeMessage::SetFloatParam(220.));

        thread::sleep(time::Duration::from_millis(2000));
        graph.pause_processing();
    } else {
        unreachable!();
    }
}
