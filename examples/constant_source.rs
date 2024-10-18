extern crate servo_media;
extern crate servo_media_auto;

use servo_media::audio::constant_source_node::ConstantSourceNodeOptions;
use servo_media::audio::gain_node::GainNodeOptions;
use servo_media::audio::node::{AudioNodeInit, AudioNodeMessage, AudioScheduledSourceNodeMessage};
use servo_media::audio::param::{ParamType, RampKind, UserAutomationEvent};
use servo_media::{ClientContextId, ServoMedia};
use std::sync::Arc;
use std::{thread, time};

fn run_example(servo_media: Arc<ServoMedia>) {
    let context = servo_media
        .create_audio_context(&ClientContextId::build(1, 1), Default::default())
        .unwrap();
    let context = context.lock().unwrap();
    let dest = context.dest_node();

    let cs = context.create_node(
        AudioNodeInit::ConstantSourceNode(ConstantSourceNodeOptions::default()),
        Default::default(),
    );

    let mut gain_options = GainNodeOptions::default();
    gain_options.gain = 0.1;
    let gain = context.create_node(
        AudioNodeInit::GainNode(gain_options.clone()),
        Default::default(),
    );

    let osc = context.create_node(
        AudioNodeInit::OscillatorNode(Default::default()),
        Default::default(),
    );

    context.connect_ports(osc.output(0), gain.input(0));
    context.connect_ports(cs.output(0), gain.param(ParamType::Gain));
    context.connect_ports(gain.output(0), dest.input(0));

    let _ = context.resume();
    context.message_node(
        osc,
        AudioNodeMessage::AudioScheduledSourceNode(AudioScheduledSourceNodeMessage::Start(0.)),
    );

    context.message_node(
        gain,
        AudioNodeMessage::AudioScheduledSourceNode(AudioScheduledSourceNodeMessage::Start(0.)),
    );

    context.message_node(
        cs,
        AudioNodeMessage::AudioScheduledSourceNode(AudioScheduledSourceNodeMessage::Start(0.)),
    );

    context.message_node(
        cs,
        AudioNodeMessage::SetParam(
            ParamType::Offset,
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 1., 1.5),
        ),
    );

    context.message_node(
        cs,
        AudioNodeMessage::SetParam(
            ParamType::Offset,
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 0.1, 3.0),
        ),
    );

    context.message_node(
        cs,
        AudioNodeMessage::SetParam(
            ParamType::Offset,
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 1., 4.5),
        ),
    );

    context.message_node(
        cs,
        AudioNodeMessage::SetParam(
            ParamType::Offset,
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 0.1, 6.0),
        ),
    );

    thread::sleep(time::Duration::from_millis(9000));
    let _ = context.close();
}

fn main() {
    ServoMedia::init::<servo_media_auto::Backend>();
    if let Ok(servo_media) = ServoMedia::get() {
        run_example(servo_media);
    } else {
        unreachable!();
    }
}
