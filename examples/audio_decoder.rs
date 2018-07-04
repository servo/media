extern crate servo_media;

use servo_media::audio::buffer_source_node::{AudioBuffer, AudioBufferSourceNodeMessage};
use servo_media::audio::decoder::AudioDecoderCallbacks;
use servo_media::audio::node::{AudioNodeMessage, AudioNodeType, AudioScheduledSourceNodeMessage};
use servo_media::ServoMedia;
use std::env;
use std::fs::File;
use std::io::Read;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::{thread, time};

fn run_example(servo_media: Arc<ServoMedia>) {
    let context = servo_media.create_audio_context(Default::default());
    let args: Vec<_> = env::args().collect();
    let filename: &str = if args.len() == 2 {
        args[1].as_ref()
    } else {
        panic!("Usage: cargo run --bin audio_decoder <file_path>")
    };
    let mut file = File::open(filename).unwrap();
    let mut bytes = vec![];
    file.read_to_end(&mut bytes).unwrap();
    // XXX Support multiple channels #55.
    let decoded_audio: Arc<Mutex<AudioBuffer>> = Arc::new(Mutex::new(AudioBuffer::new(1, 0)));
    let progress = decoded_audio.clone();
    let (sender, receiver) = mpsc::channel();
    let callbacks = AudioDecoderCallbacks::new()
        .eos(move || {
            sender.send(()).unwrap();
        })
        .error(|| {
            eprintln!("Error decoding audio");
        })
        .progress(move |buffer| {
            progress
                .lock()
                .unwrap()
                .buffers[0]
                .extend_from_slice((*buffer).as_ref());
        })
        .build();
    context.decode_audio_data(bytes.to_vec(), callbacks);
    println!("Decoding audio");
    receiver.recv().unwrap();
    println!("Audio decoded");
    let buffer_source = context.create_node(AudioNodeType::AudioBufferSourceNode(Default::default()));
    let dest = context.dest_node();
    context.connect_ports(buffer_source.output(0), dest.input(0));
    context.message_node(
        buffer_source,
        AudioNodeMessage::AudioScheduledSourceNode(AudioScheduledSourceNodeMessage::Start(0.)),
    );
    context.message_node(
        buffer_source,
        AudioNodeMessage::AudioBufferSourceNode(AudioBufferSourceNodeMessage::SetBuffer(
            Some(decoded_audio),
        )),
    );
    let _ = context.resume();
    thread::sleep(time::Duration::from_millis(5000));
    let _ = context.close();
}

fn main() {
    if let Ok(servo_media) = ServoMedia::get() {
        run_example(servo_media);
    } else {
        unreachable!()
    }
}
