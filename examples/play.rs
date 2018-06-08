extern crate servo_media;

use servo_media::audio::gain_node::{GainNodeMessage, GainNodeOptions};
use servo_media::audio::node::{AudioNodeMessage, AudioNodeType};
use servo_media::audio::oscillator_node::OscillatorNodeMessage;
use servo_media::audio::param::UserAutomationEvent;
use servo_media::ServoMedia;
use std::{thread, time};

fn main() {
    if let Ok(servo_media) = ServoMedia::get() {
        let mut graph = servo_media.create_audio_graph();
        let osc = graph.create_node(AudioNodeType::OscillatorNode(Default::default()));
        let mut options = GainNodeOptions::default();
        options.gain = 0.5;
        let gain = graph.create_node(AudioNodeType::GainNode(options));
        let dest = graph.dest_node();
        graph.connect_ports(osc.output(0), gain.input(0));
        graph.connect_ports(gain.output(0), dest.input(0));
        graph.message_node(
            osc,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::Start(0.)),
        );
        graph.message_node(
            osc,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::Stop(3.)),
        );
        assert_eq!(graph.current_time(), 0.);
        graph.resume();
        // 0.5s: Set frequency to 110Hz
        graph.message_node(
            osc,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::SetFrequency(
                UserAutomationEvent::SetValueAtTime(110., 0.5),
            )),
        );
        // 1s: Set frequency to 220Hz
        graph.message_node(
            osc,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::SetFrequency(
                UserAutomationEvent::SetValueAtTime(220., 1.),
            )),
        );
        // 0.75s: Set gain to 0.25
        graph.message_node(
            gain,
            AudioNodeMessage::GainNode(GainNodeMessage::SetGain(
                UserAutomationEvent::SetValueAtTime(0.25, 0.75),
            )),
        );
        thread::sleep(time::Duration::from_millis(1200));
        // 1.2s: Suspend processing
        graph.suspend();
        thread::sleep(time::Duration::from_millis(500));
        // 1.7s: Resume processing
        graph.resume();
        let current_time = graph.current_time();
        assert!(current_time > 0.);
        // Leave some time to enjoy the silence after stopping the
        // oscillator node.
        thread::sleep(time::Duration::from_millis(5000));
        // And check that we keep incrementing playback time.
        assert!(current_time < graph.current_time());
        graph.close();
    } else {
        unreachable!();
    }
}
