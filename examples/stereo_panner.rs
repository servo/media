extern crate servo_media;
extern crate servo_media_auto;

use servo_media::audio::node::{AudioNodeInit, AudioNodeMessage, AudioScheduledSourceNodeMessage};
use servo_media::audio::param::{ParamType, RampKind, UserAutomationEvent};
use servo_media::audio::stereo_panner::StereoPannerOptions;
use servo_media::{ClientContextId, ServoMedia};
use std::sync::Arc;
use std::{thread, time};

fn run_example(servo_media: Arc<ServoMedia>) {
    let client_context_id = ClientContextId::build(1, 1);
    let context = servo_media.create_audio_context(&client_context_id, Default::default());
    {
        let context = context.lock().unwrap();
        let dest = context.dest_node();
        let osc = context.create_node(
            AudioNodeInit::OscillatorNode(Default::default()),
            Default::default(),
        );
        let mut options = StereoPannerOptions::default();
        options.pan = 0.;
        let pan = context.create_node(AudioNodeInit::StereoPannerNode(options), Default::default());
        context.connect_ports(osc.output(0), pan.input(0));
        context.connect_ports(pan.output(0), dest.input(0));
        let _ = context.resume();
        context.message_node(
            osc,
            AudioNodeMessage::AudioScheduledSourceNode(AudioScheduledSourceNodeMessage::Start(0.)),
        );
        // 2s: Set pan to -1
        context.message_node(
            pan,
            AudioNodeMessage::SetParam(
                ParamType::Pan,
                UserAutomationEvent::SetValueAtTime(-1., 2.),
            ),
        );
        // 4s: Linearly ramp pan to 0
        context.message_node(
            pan,
            AudioNodeMessage::SetParam(
                ParamType::Pan,
                UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 0., 4.),
            ),
        );
        // 6s: Linearly ramp pan to 1
        context.message_node(
            pan,
            AudioNodeMessage::SetParam(
                ParamType::Pan,
                UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 1., 6.),
            ),
        );
        thread::sleep(time::Duration::from_millis(5000));
        let _ = context.close();
    }
    ServoMedia::get()
        .unwrap()
        .shutdown_audio_context(&client_context_id, context);
}

fn main() {
    ServoMedia::init::<servo_media_auto::Backend>();
    if let Ok(servo_media) = ServoMedia::get() {
        run_example(servo_media);
    } else {
        unreachable!();
    }
}
