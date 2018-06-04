extern crate servo_media;

use servo_media::audio::node::{AudioNodeMessage, AudioNodeType};
use servo_media::audio::oscillator_node::OscillatorNodeMessage;
use servo_media::audio::param::{RampKind, UserAutomationEvent};
use servo_media::ServoMedia;
use std::{thread, time};

fn main() {
    if let Ok(servo_media) = ServoMedia::get() {
        let mut graph = servo_media.create_audio_graph();
        graph.create_node(AudioNodeType::OscillatorNode(Default::default()));
        graph.resume();
        graph.message_node(
            0,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::Start(0.)),
        );
        // 0.1s: Set frequency to 110Hz
        graph.message_node(
            0,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::SetFrequency(
                UserAutomationEvent::SetValueAtTime(110., 0.1),
            )),
        );
        // 0.3s: Start increasing frequency to 440Hz exponentially with a time constant of 1
        graph.message_node(
            0,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::SetFrequency(
                UserAutomationEvent::SetTargetAtTime(440., 0.3, 1.),
            )),
        );
        // 1.5s: Start increasing frequency to 1760Hz exponentially
        // this event effectively doesn't happen, but instead sets a starting point
        // for the next ramp event
        graph.message_node(
            0,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::SetFrequency(
                UserAutomationEvent::SetTargetAtTime(1760., 1.5, 0.1),
            )),
        );
        // 1.5s - 3s Linearly ramp down from the previous event (1.5s) to 110Hz
        graph.message_node(
            0,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::SetFrequency(
                UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 110., 3.0),
            )),
        );
        thread::sleep(time::Duration::from_millis(5000));
    } else {
        unreachable!();
    }
}
