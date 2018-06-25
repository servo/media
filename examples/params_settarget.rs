extern crate servo_media;

use servo_media::audio::node::{AudioNodeMessage, AudioNodeType};
use servo_media::audio::oscillator_node::OscillatorNodeMessage;
use servo_media::audio::param::{RampKind, UserAutomationEvent};
use servo_media::ServoMedia;
use std::sync::Arc;
use std::{thread, time};

fn run_example(servo_media: Arc<ServoMedia>) {
    let context = servo_media.create_audio_context(Default::default());
    let dest = context.dest_node();
    let osc = context.create_node(AudioNodeType::OscillatorNode(Default::default()));
    context.connect_ports(osc.output(0), dest.input(0));
    context.resume();
    context.message_node(
        osc,
        AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::Start(0.)),
    );
    // 0.1s: Set frequency to 110Hz
    context.message_node(
        osc,
        AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::SetFrequency(
            UserAutomationEvent::SetValueAtTime(110., 0.1),
        )),
    );
    // 0.3s: Start increasing frequency to 440Hz exponentially with a time constant of 1
    context.message_node(
        osc,
        AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::SetFrequency(
            UserAutomationEvent::SetTargetAtTime(440., 0.3, 1.),
        )),
    );
    // 1.5s: Start increasing frequency to 1760Hz exponentially
    // this event effectively doesn't happen, but instead sets a starting point
    // for the next ramp event
    context.message_node(
        osc,
        AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::SetFrequency(
            UserAutomationEvent::SetTargetAtTime(1760., 1.5, 0.1),
        )),
    );
    // 1.5s - 3s Linearly ramp down from the previous event (1.5s) to 110Hz
    context.message_node(
        osc,
        AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::SetFrequency(
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 110., 3.0),
        )),
    );
    thread::sleep(time::Duration::from_millis(5000));
}

fn main() {
    if let Ok(servo_media) = ServoMedia::get() {
        run_example(servo_media);
    } else {
        unreachable!();
    }
}
