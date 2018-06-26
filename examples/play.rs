extern crate servo_media;

use servo_media::audio::gain_node::{GainNodeMessage, GainNodeOptions};
use servo_media::audio::node::{AudioNodeMessage, AudioNodeType, AudioScheduledSourceNodeMessage};
use servo_media::audio::oscillator_node::OscillatorNodeMessage;
use servo_media::audio::param::UserAutomationEvent;
use servo_media::ServoMedia;
use std::sync::Arc;
use std::{thread, time};

fn run_example(servo_media: Arc<ServoMedia>) {
    let context = servo_media.create_audio_context(Default::default());
    let osc = context.create_node(AudioNodeType::OscillatorNode(Default::default()));
    let mut options = GainNodeOptions::default();
    options.gain = 0.5;
    let gain = context.create_node(AudioNodeType::GainNode(options));
    let dest = context.dest_node();
    context.connect_ports(osc.output(0), gain.input(0));
    context.connect_ports(gain.output(0), dest.input(0));
    context.message_node(
        osc,
        AudioNodeMessage::AudioScheduledSourceNode(AudioScheduledSourceNodeMessage::Start(0.)),
    );
    context.message_node(
        osc,
        AudioNodeMessage::AudioScheduledSourceNode(AudioScheduledSourceNodeMessage::Stop(3.)),
    );
    assert_eq!(context.current_time(), 0.);
    let _ = context.resume();
    // 0.5s: Set frequency to 110Hz
    context.message_node(
        osc,
        AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::SetFrequency(
            UserAutomationEvent::SetValueAtTime(110., 0.5),
        )),
    );
    // 1s: Set frequency to 220Hz
    context.message_node(
        osc,
        AudioNodeMessage::OscillatorNode(OscillatorNodeMessage::SetFrequency(
            UserAutomationEvent::SetValueAtTime(220., 1.),
        )),
    );
    // 0.75s: Set gain to 0.25
    context.message_node(
        gain,
        AudioNodeMessage::GainNode(GainNodeMessage::SetGain(
            UserAutomationEvent::SetValueAtTime(0.25, 0.75),
        )),
    );
    thread::sleep(time::Duration::from_millis(1200));
    // 1.2s: Suspend processing
    let _ = context.suspend();
    thread::sleep(time::Duration::from_millis(500));
    // 1.7s: Resume processing
    let _ = context.resume();
    let current_time = context.current_time();
    assert!(current_time > 0.);
    // Leave some time to enjoy the silence after stopping the
    // oscillator node.
    thread::sleep(time::Duration::from_millis(5000));
    // And check that we keep incrementing playback time.
    assert!(current_time < context.current_time());
    let _ = context.close();
}

fn main() {
    if let Ok(servo_media) = ServoMedia::get() {
        run_example(servo_media);
    } else {
        unreachable!();
    }
}
