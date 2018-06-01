extern crate servo_media;

use servo_media::audio::gain_node::{GainNodeMessage, GainNodeOptions};
use servo_media::audio::node::{AudioNodeMessage, AudioNodeType};
use servo_media::audio::oscillator_node::OscillatorNodeMessage;
use servo_media::audio::param::{RampKind, UserAutomationEvent};
use servo_media::ServoMedia;
use std::{thread, time};

fn main() {
    if let Ok(servo_media) = ServoMedia::get() {
        let mut graph = servo_media.create_audio_graph();
        graph.create_node(AudioNodeType::OscillatorNode(Default::default()));
        let mut options = GainNodeOptions::default();
        options.gain = 0.5;
        graph.create_node(AudioNodeType::GainNode(options));
        graph.message_node(
            0,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::Start(0.)),
        );
        graph.message_node(
            0,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::Stop(3.)),
        );
        assert_eq!(graph.current_time(), 0.);
        graph.resume();
        graph.message_node(
            0,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::SetFrequency(
                UserAutomationEvent::SetValueAtTime(110., 0.5),
            )),
        );
        graph.message_node(
            0,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::SetFrequency(
                UserAutomationEvent::SetValueAtTime(220., 1.),
            )),
        );
        graph.message_node(
            1,
            AudioNodeMessage::GainNode(GainNodeMessage::SetGain(
                UserAutomationEvent::SetValueAtTime(0.25, 0.75),
            )),
        );
        graph.message_node(
            1,
            AudioNodeMessage::GainNode(GainNodeMessage::SetGain(
                UserAutomationEvent::RampToValueAtTime(RampKind::Exponential, 1., 1.5),
            )),
        );
        graph.message_node(
            0,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::SetFrequency(
                UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 880., 1.75),
            )),
        );
        graph.message_node(
            0,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::SetFrequency(
                UserAutomationEvent::RampToValueAtTime(RampKind::Exponential, 110., 2.5),
            )),
        );
        thread::sleep(time::Duration::from_millis(3000));
        graph.suspend();
        thread::sleep(time::Duration::from_millis(500));
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
