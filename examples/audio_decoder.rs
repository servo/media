extern crate servo_media;

use servo_media::audio::buffer_source_node::AudioBufferSourceNodeMessage;
use servo_media::audio::decoder::AudioDecoderMsg;
use servo_media::audio::node::{AudioNodeMessage, AudioNodeType};
use servo_media::ServoMedia;
use std::env;
use std::fs::File;
use std::io::Read;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::Builder;
use std::{thread, time};

fn run_example(servo_media: Arc<ServoMedia>) {
    let mut graph = servo_media.create_audio_graph();
    let args: Vec<_> = env::args().collect();
    let filename: &str = if args.len() == 2 {
        args[1].as_ref()
    } else {
        panic!("Usage: cargo run --bin audio_decoder <file_path>")
    };
    let mut file = File::open(filename).unwrap();
    let mut bytes = vec![];
    file.read_to_end(&mut bytes).unwrap();
    let (sender, receiver) = mpsc::channel();
    graph.decode_audio_data(bytes.to_vec(), sender);
    let (sender2, receiver2) = mpsc::channel();
    Builder::new()
        .name("AudioDecoder receiver".to_owned())
        .spawn(move || {
            let mut decoded_audio = Vec::new();
            loop {
                match receiver.recv().unwrap() {
                    AudioDecoderMsg::Eos => {
                        let _ = sender2.send(decoded_audio);
                        break;
                    }
                    AudioDecoderMsg::Error => break,
                    AudioDecoderMsg::Progress(progress) => {
                        decoded_audio.extend_from_slice(&progress);
                    }
                }
            }
        })
        .unwrap();
    println!("Decoding");
    let decoded_audio = receiver2.recv().unwrap();
    println!("Audio decoded");
    let buffer_source = graph.create_node(AudioNodeType::AudioBufferSourceNode);
    let dest = graph.dest_node();
    graph.connect_ports(buffer_source.output(0), dest.input(0));
    graph.message_node(
        buffer_source,
        AudioNodeMessage::AudioBufferSourceNode(AudioBufferSourceNodeMessage::Start(0.)),
    );
    graph.message_node(
        buffer_source,
        AudioNodeMessage::AudioBufferSourceNode(AudioBufferSourceNodeMessage::SetBuffer(
            decoded_audio,
        )),
    );
    graph.resume();
    thread::sleep(time::Duration::from_millis(5000));
    graph.close();
}

fn main() {
    if let Ok(servo_media) = ServoMedia::get() {
        run_example(servo_media);
    } else {
        unreachable!()
    }
}
