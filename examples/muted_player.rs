extern crate ipc_channel;
extern crate servo_media;
extern crate servo_media_auto;

use ipc_channel::ipc;
use servo_media::player::context::{GlApi, GlContext, NativeDisplay, PlayerGLContext};
use servo_media::player::{PlayerEvent, StreamType};
use servo_media::{ClientContextId, ServoMedia};
use std::env;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::sync::Arc;

struct PlayerContextDummy();
impl PlayerGLContext for PlayerContextDummy {
    fn get_gl_context(&self) -> GlContext {
        return GlContext::Unknown;
    }

    fn get_native_display(&self) -> NativeDisplay {
        return NativeDisplay::Unknown;
    }

    fn get_gl_api(&self) -> GlApi {
        return GlApi::None;
    }
}

fn run_example(servo_media: Arc<ServoMedia>) {
    let args: Vec<_> = env::args().collect();
    let default = "./examples/resources/viper_cut.ogg";
    let filename: &str = if args.len() == 2 {
        args[1].as_ref()
    } else if Path::new(default).exists() {
        default
    } else {
        panic!("Usage: cargo run --bin player <file_path>")
    };

    let context_id = &ClientContextId::build(1, 1);
    let (sender, receiver) = ipc::channel().unwrap();
    let player = servo_media.create_player(
        &context_id,
        StreamType::Seekable,
        sender,
        None,
        None,
        Box::new(PlayerContextDummy()),
    );

    let path = Path::new(filename);
    let display = path.display();

    let file = match File::open(&path) {
        Err(why) => panic!("couldn't open {}: {}", display, why),
        Ok(file) => file,
    };

    if let Ok(metadata) = file.metadata() {
        player
            .lock()
            .unwrap()
            .set_input_size(metadata.len())
            .unwrap();
    }

    let mut buf_reader = BufReader::new(file);
    let mut buffer = [0; 1024];

    loop {
        match buf_reader.read(&mut buffer[..]) {
            Ok(0) => {
                println!("Finished pushing data");
                break;
            },
            Ok(size) => player
                .lock()
                .unwrap()
                .push_data(Vec::from(&buffer[0..size]))
                .unwrap(),
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            },
        }
    }

    player.lock().unwrap().play().unwrap();

    let mut muted = false;
    while let Ok(event) = receiver.recv() {
        match event {
            PlayerEvent::EndOfStream => {
                println!("\nEOF");
                break;
            },
            PlayerEvent::Error(ref s) => {
                println!("\nError {:?}", s);
                break;
            },
            PlayerEvent::MetadataUpdated(ref m) => {
                println!("\nMetadata updated! {:?}", m);
            },
            PlayerEvent::DurationChanged(d) => {
                println!("\nDuration changed! {:?}", d);
            },
            PlayerEvent::StateChanged(ref s) => {
                println!("\nPlayer state changed to {:?}", s);
            },
            PlayerEvent::VideoFrameUpdated => eprint!("."),
            PlayerEvent::PositionChanged(p) => {
                if p as u64 == 2 && !muted {
                    println!("\nPosition is at 2sec, muting, 1 second of silence incoming");
                    servo_media.mute(&context_id, true);
                    muted = true;
                } else if p as u64 == 3 && muted {
                    println!("\nPosition is at 3sec, unmuting");
                    servo_media.mute(&context_id, false);
                    muted = false;
                }
            },
            PlayerEvent::SeekData(_, _) => {},
            PlayerEvent::SeekDone(_) => {},
            PlayerEvent::NeedData => println!("\nNeedData"),
            PlayerEvent::EnoughData => println!("\nEnoughData"),
        }
    }
}

fn main() {
    ServoMedia::init::<servo_media_auto::Backend>();
    let servo_media = ServoMedia::get();
    run_example(servo_media);
}
