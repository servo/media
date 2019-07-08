extern crate servo_media;
extern crate servo_media_auto;

use servo_media::audio::node::{AudioNodeInit, AudioNodeMessage, AudioScheduledSourceNodeMessage};
use servo_media::audio::oscillator_node::OscillatorNodeOptions;
use servo_media::audio::wave_shaper_node::OverSampleType;
use servo_media::audio::wave_shaper_node::{WaveShaperNodeMessage, WaveShaperNodeOptions};

use servo_media::{ClientContextId, ServoMedia};
use std::sync::Arc;
use std::{thread, time};

fn run_example(servo_media: Arc<ServoMedia>) {
    let id = ClientContextId::build(1, 1);
    let context = servo_media.create_audio_context(&id, Default::default());

    {
        let context = context.lock().unwrap();

        let dest = context.dest_node();
        let osc = context.create_node(
            AudioNodeInit::OscillatorNode(OscillatorNodeOptions::default()),
            Default::default(),
        );
        let wsh = context.create_node(
            AudioNodeInit::WaveShaperNode(WaveShaperNodeOptions {
                curve: Some(vec![0., 0., 0., 4., 6.]),
                oversample: OverSampleType::None,
            }),
            Default::default(),
        );

        context.connect_ports(osc.output(0), dest.input(0));
        let _ = context.resume();
        context.message_node(
            osc,
            AudioNodeMessage::AudioScheduledSourceNode(AudioScheduledSourceNodeMessage::Start(0.)),
        );

        println!("raw oscillator");
        thread::sleep(time::Duration::from_millis(2000));

        println!("oscillator through waveshaper");
        context.disconnect_output(osc.output(0));
        context.connect_ports(osc.output(0), wsh.input(0));
        context.connect_ports(wsh.output(0), dest.input(0));
        thread::sleep(time::Duration::from_millis(2000));

        println!("oscillator through waveshaper with no curve (should be same as raw oscillator)");
        context.message_node(
            wsh,
            AudioNodeMessage::WaveShaperNode(WaveShaperNodeMessage::SetCurve(None)),
        );
        thread::sleep(time::Duration::from_millis(2000));
    }
}

fn main() {
    ServoMedia::init::<servo_media_auto::Backend>();
    if let Ok(servo_media) = ServoMedia::get() {
        run_example(servo_media);
    } else {
        unreachable!();
    }
}
