extern crate servo_media;

use servo_media::audio::panner_node::PannerNodeOptions;
use servo_media::audio::node::{AudioNodeInit, AudioNodeMessage, AudioScheduledSourceNodeMessage};
use servo_media::audio::param::{ParamDir, ParamType, RampKind, UserAutomationEvent};
use servo_media::ServoMedia;
use std::sync::Arc;
use std::{thread, time};

fn run_example(servo_media: Arc<ServoMedia>) {
    let context = servo_media.create_audio_context(Default::default());
    let dest = context.dest_node();
    let listener = context.listener();
    let osc = context.create_node(AudioNodeInit::OscillatorNode(Default::default()));
    let mut options = PannerNodeOptions::default();
    options.cone_outer_angle = 0.;
    options.position_x = 100.;
    let panner = context.create_node(AudioNodeInit::PannerNode(options));
    context.connect_ports(osc.output(0), panner.input(0));
    context.connect_ports(panner.output(0), dest.input(0));
    let _ = context.resume();
    context.message_node(
        osc,
        AudioNodeMessage::AudioScheduledSourceNode(AudioScheduledSourceNodeMessage::Start(0.)),
    );
    context.message_node(
        panner,
        AudioNodeMessage::SetParam(
            ParamType::Orientation(ParamDir::X),
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 0., 1.0),
        ),
    );
    context.message_node(
        panner,
        AudioNodeMessage::SetParam(
            ParamType::Orientation(ParamDir::Z),
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 1., 1.0),
        ),
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
