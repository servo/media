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
        let osc = graph.create_node(AudioNodeType::OscillatorNode(Default::default()));
        let mut options = GainNodeOptions::default();
        options.gain = 0.5;
        let gain = graph.create_node(AudioNodeType::GainNode(options));
        graph.resume();
        graph.message_node(
            osc,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::Start(0.)),
        );
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
        // 0.75s - 1.5s: Exponentially ramp gain to 1
        graph.message_node(
            gain,
            AudioNodeMessage::GainNode(GainNodeMessage::SetGain(
                UserAutomationEvent::RampToValueAtTime(RampKind::Exponential, 1., 1.5),
            )),
        );
        // 0.75s - 1.75s: Linearly ramp frequency to 880Hz
        graph.message_node(
            osc,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::SetFrequency(
                UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 880., 1.75),
            )),
        );
        // 1.75s - 2.5s: Exponentially ramp frequency to 110Hz
        graph.message_node(
            osc,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::SetFrequency(
                UserAutomationEvent::RampToValueAtTime(RampKind::Exponential, 110., 2.5),
            )),
        );

        // 2.75s: Exponentially approach 110Hz
        graph.message_node(
            osc,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::SetFrequency(
                UserAutomationEvent::SetTargetAtTime(1100., 2.75, 1.1),
            )),
        );
        // 3.3s: But actually stop at 3.3Hz and hold
        graph.message_node(
            osc,
            AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::SetFrequency(
                UserAutomationEvent::CancelAndHoldAtTime(3.3),
            )),
        );
        thread::sleep(time::Duration::from_millis(5000));
    } else {
        unreachable!();
    }
}
