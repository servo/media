extern crate servo_media;

use servo_media::audio::gain_node::GainNodeOptions;
use servo_media::audio::param::{RampKind, UserAutomationEvent};
use servo_media::audio::node::{AudioNodeMessage, AudioNodeType};
use servo_media::ServoMedia;
use std::{thread, time};

fn main() {
    if let Ok(servo_media) = ServoMedia::get() {
        let mut graph = servo_media.create_audio_graph();
        graph.create_node(AudioNodeType::OscillatorNode(Default::default()));
        let mut options = GainNodeOptions::default();
        options.gain = 0.5;
        graph.create_node(AudioNodeType::GainNode(options));
        assert_eq!(graph.current_time(), 0.);
        graph.resume();
        // change frequency at 0.5s and 1s, then ramp up linearly till 1.7s, then ramp down till 2.5s
        // change gain at 0.75s, then ramp to full gain reached at 1.5s
        graph.message_node(0, AudioNodeMessage::SetAudioParamEvent(UserAutomationEvent::SetValueAtTime(110., 0.5)));
        graph.message_node(0, AudioNodeMessage::SetAudioParamEvent(UserAutomationEvent::SetValueAtTime(220., 1.)));
        graph.message_node(1, AudioNodeMessage::SetAudioParamEvent(UserAutomationEvent::SetValueAtTime(0.25, 0.75)));
        graph.message_node(1, AudioNodeMessage::SetAudioParamEvent(UserAutomationEvent::RampToValueAtTime(RampKind::Exponential, 1., 1.5)));
        graph.message_node(0, AudioNodeMessage::SetAudioParamEvent(UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 880., 1.75)));
        graph.message_node(0, AudioNodeMessage::SetAudioParamEvent(UserAutomationEvent::RampToValueAtTime(RampKind::Exponential, 110., 2.5)));
        thread::sleep(time::Duration::from_millis(3000));
        graph.suspend();
        thread::sleep(time::Duration::from_millis(500));
        graph.resume();
        assert!(graph.current_time() != 0.);
        thread::sleep(time::Duration::from_millis(500));
        graph.close();
    } else {
        unreachable!();
    }
}
