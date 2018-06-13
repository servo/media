extern crate servo_media;

use servo_media::ServoMedia;
use std::env;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;
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
    graph.decode_audio_data(bytes.to_vec());
    thread::sleep(time::Duration::from_millis(4000));
    graph.close();
}

fn main() {
    if let Ok(servo_media) = ServoMedia::get() {
        run_example(servo_media);
    } else {
        unreachable!()
    }
}
