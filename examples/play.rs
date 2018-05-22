extern crate servo_media;

use servo_media::audio::node::AudioNodeType;
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
        thread::sleep(time::Duration::from_millis(5000));
        graph.pause_processing();
    } else {
        unreachable!();
    }
}
