extern crate servo_media;

use servo_media::audio::oscillator_node::OscillatorNodeOptions;
use servo_media::audio::gain_node::GainNodeOptions;
use servo_media::audio::node::{
    AudioNodeInit, AudioNodeMessage, AudioScheduledSourceNodeMessage,
};
use servo_media::audio::param::{ParamType, RampKind, UserAutomationEvent};
use servo_media::ServoMedia;
use std::sync::Arc;
use std::{thread, time};

fn run_example(servo_media: Arc<ServoMedia>) {
    let context = servo_media.create_audio_context(Default::default());
    let mut options = OscillatorNodeOptions::default();
    options.freq = 2.0;
    let lfo = context.create_node(AudioNodeInit::OscillatorNode(options));
    let osc = context.create_node(AudioNodeInit::OscillatorNode(Default::default()));
    let mut options = GainNodeOptions::default();
    options.gain = 100.;
    let gain = context.create_node(AudioNodeInit::GainNode(options));
    let dest = context.dest_node();
    context.connect_ports(lfo.output(0), gain.input(0));
    context.connect_ports(gain.output(0), osc.param(ParamType::Frequency));
    context.connect_ports(osc.output(0), dest.input(0));
    let _ = context.resume();
    context.message_node(
        osc,
        AudioNodeMessage::AudioScheduledSourceNode(AudioScheduledSourceNodeMessage::Start(0.)),
    );
    context.message_node(
        lfo,
        AudioNodeMessage::AudioScheduledSourceNode(AudioScheduledSourceNodeMessage::Start(0.)),
    );
    thread::sleep(time::Duration::from_millis(3000));
    // 0.75s - 1.75s: Linearly ramp frequency to 880Hz
    context.message_node(
        gain,
        AudioNodeMessage::SetParam(
            ParamType::Gain,
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 0., 6.),
        ),
    );

    thread::sleep(time::Duration::from_millis(3000));
    let _ = context.close();
}

fn main() {
    if let Ok(servo_media) = ServoMedia::get() {
        run_example(servo_media);
    } else {
        unreachable!();
    }
}
