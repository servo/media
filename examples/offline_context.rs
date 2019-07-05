extern crate servo_media;
extern crate servo_media_auto;

use servo_media::audio::block::FRAMES_PER_BLOCK_USIZE;
use servo_media::audio::buffer_source_node::AudioBufferSourceNodeMessage;
use servo_media::audio::context::{AudioContextOptions, OfflineAudioContextOptions};
use servo_media::audio::node::{AudioNodeInit, AudioNodeMessage, AudioScheduledSourceNodeMessage};
use servo_media::{ClientContextId, ServoMedia};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::{thread, time};

fn run_example(servo_media: Arc<ServoMedia>) {
    // Create offline context to process 1024 blocks of a oscillator node produced
    // sine wave.
    let processed_audio = Arc::new(Mutex::new(Vec::new()));
    let processed_audio_ = processed_audio.clone();
    let client_context_id = ClientContextId::build(1, 1);
    let mut options = <OfflineAudioContextOptions>::default();
    options.channels = 2;
    options.length = 1024 * FRAMES_PER_BLOCK_USIZE;
    let options = AudioContextOptions::OfflineAudioContext(options);
    let context = servo_media.create_audio_context(&client_context_id, options);
    {
        let context = context.lock().unwrap();
        let (sender, receiver) = mpsc::channel();
        let sender = Mutex::new(sender);
        context.set_eos_callback(Box::new(move |buffer| {
            processed_audio
                .lock()
                .unwrap()
                .extend_from_slice((*buffer).as_ref());
            sender.lock().unwrap().send(()).unwrap();
        }));
        let osc = context.create_node(
            AudioNodeInit::OscillatorNode(Default::default()),
            Default::default(),
        );
        let dest = context.dest_node();
        context.connect_ports(osc.output(0), dest.input(0));
        context.message_node(
            osc,
            AudioNodeMessage::AudioScheduledSourceNode(AudioScheduledSourceNodeMessage::Start(0.)),
        );
        let _ = context.resume();
        // Block until we processed the data.
        receiver.recv().unwrap();
        // Close offline context.
        let _ = context.close();
    }
    ServoMedia::get()
        .unwrap()
        .shutdown_audio_context(&client_context_id, context);

    let client_context_id = ClientContextId::build(1, 2);
    let context = servo_media.create_audio_context(&client_context_id, Default::default());
    // Create audio context to play the processed audio.
    {
        let context = context.lock().unwrap();
        let buffer_source = context.create_node(
            AudioNodeInit::AudioBufferSourceNode(Default::default()),
            Default::default(),
        );
        let dest = context.dest_node();
        context.connect_ports(buffer_source.output(0), dest.input(0));
        context.message_node(
            buffer_source,
            AudioNodeMessage::AudioScheduledSourceNode(AudioScheduledSourceNodeMessage::Start(0.)),
        );
        context.message_node(
            buffer_source,
            AudioNodeMessage::AudioBufferSourceNode(AudioBufferSourceNodeMessage::SetBuffer(Some(
                processed_audio_.lock().unwrap().to_vec().into(),
            ))),
        );
        let _ = context.resume();
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
        unreachable!()
    }
}
