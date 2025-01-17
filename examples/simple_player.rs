extern crate ipc_channel;
extern crate servo_media;
extern crate servo_media_auto;

use ipc_channel::ipc;
use servo_media::player::context::{GlApi, GlContext, NativeDisplay, PlayerGLContext};
use servo_media::player::{PlayerEvent, StreamType};
use servo_media::{ClientContextId, ServoMedia};
use std::env;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

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

    let devices = servo_media
        .get_device_monitor()
        .enumerate_devices()
        .unwrap();
    for device in devices {
        println!("{:?}", device);
    }

    let (sender, receiver) = ipc::channel().unwrap();
    let player = servo_media.create_player(
        &ClientContextId::build(1, 1),
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

    let player_clone = Arc::clone(&player);
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();
    let (seek_sender, seek_receiver) = mpsc::channel();
    let t = thread::spawn(move || {
        let player = &player_clone;
        let mut buf_reader = BufReader::new(file);
        let mut buffer = [0; 1024];
        let mut read = |offset| {
            if buf_reader.seek(SeekFrom::Start(offset)).is_err() {
                eprintln!("BufReader - Could not seek to {:?}", offset);
            }

            while !shutdown_clone.load(Ordering::Relaxed) {
                match buf_reader.read(&mut buffer[..]) {
                    Ok(0) => {
                        println!("Finished pushing data");
                        break;
                    }
                    Ok(size) => player
                        .lock()
                        .unwrap()
                        .push_data(Vec::from(&buffer[0..size]))
                        .unwrap(),
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        break;
                    }
                }
            }
        };

        loop {
            if let Ok(position) = seek_receiver.try_recv() {
                read(position);
            }

            if shutdown_clone.load(Ordering::Relaxed) {
                break;
            }
        }
    });

    player.lock().unwrap().play().unwrap();
    seek_sender.send(0).unwrap();

    let mut seek_requested = false;
    while let Ok(event) = receiver.recv() {
        match event {
            PlayerEvent::EndOfStream => {
                println!("\nEOF");
                break;
            }
            PlayerEvent::Error(ref s) => {
                println!("\nError {:?}", s);
                break;
            }
            PlayerEvent::MetadataUpdated(ref m) => {
                println!("\nMetadata updated! {:?}", m);
            }
            PlayerEvent::StateChanged(ref s) => {
                println!("\nPlayer state changed to {:?}", s);
            }
            PlayerEvent::VideoFrameUpdated => eprint!("."),
            PlayerEvent::PositionChanged(p) => {
                let player = player.lock().unwrap();
                if p == 4 && !seek_requested {
                    println!("\nPosition changed to 4sec, seeking back to 0sec");
                    if let Err(e) = player.seek(0.) {
                        eprintln!("{:?}", e);
                    } else {
                        seek_requested = true;
                    }
                }
            }
            PlayerEvent::SeekData(p, seek_lock) => {
                println!("\nSeek requested to position {:?}", p);
                seek_sender.send(p).unwrap();
                seek_lock.unlock(true);
            }
            PlayerEvent::SeekDone(p) => println!("\nSeeked to {:?}", p),
            PlayerEvent::NeedData => println!("\nNeedData"),
            PlayerEvent::EnoughData => println!("\nEnoughData"),
        }
    }

    shutdown.store(true, Ordering::Relaxed);
    let _ = t.join();
}

fn main() {
    ServoMedia::init::<servo_media_auto::Backend>();
    let servo_media = ServoMedia::get();
    run_example(servo_media);
}
