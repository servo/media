extern crate servo_media;

use servo_media::audio::gain_node::GainNodeOptions;
use servo_media::audio::node::{AudioNodeMessage, AudioNodeType};
use servo_media::ServoMedia;
use std::{thread, time};

fn main() {
    if let Ok(servo_media) = ServoMedia::get() {
        let mut graph = servo_media.create_audio_graph();
        graph.create_node(AudioNodeType::OscillatorNode(Default::default()));
        let mut options = GainNodeOptions::default();
        options.gain = 0.5;
        graph.create_node(AudioNodeType::GainNode(options));
        assert_eq!(graph.current_time(), 0.);
        graph.resume();
        thread::sleep(time::Duration::from_millis(2000));
        graph.message_node(0, AudioNodeMessage::SetFloatParam(220.));
        thread::sleep(time::Duration::from_millis(1000));
        graph.suspend();
        thread::sleep(time::Duration::from_millis(1000));
        graph.resume();
        assert!(graph.current_time() != 0.);
        thread::sleep(time::Duration::from_millis(1000));
        graph.close();
    } else {
        unreachable!();
    }
}
