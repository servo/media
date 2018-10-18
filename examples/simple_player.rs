extern crate ipc_channel;
extern crate servo_media;

use ipc_channel::ipc::{self, IpcSender};
use servo_media::player::{PlayerEvent, StreamType};
use servo_media::ServoMedia;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn run_example(servo_media: Arc<ServoMedia>) {
    let player = Arc::new(Mutex::new(servo_media.create_player()));
    let args: Vec<_> = env::args().collect();
    let default = "./examples/resources/viper_cut.ogg";
    let filename: &str = if args.len() == 2 {
        args[1].as_ref()
    } else if Path::new(default).exists() {
        default
    } else {
        panic!("Usage: cargo run --bin player <file_path>")
    };

    let (sender, receiver) = ipc::channel().unwrap();
    player
        .lock()
        .unwrap()
        .register_event_handler(sender)
        .unwrap();

    let path = Path::new(filename);
    let display = path.display();

    let file = match File::open(&path) {
        Err(why) => panic!("couldn't open {}: {}", display, why.description()),
        Ok(file) => file,
    };

    if let Ok(metadata) = file.metadata() {
        player
            .lock()
            .unwrap()
            .set_input_size(metadata.len())
            .unwrap();
    }

    player
        .lock()
        .unwrap()
        .set_stream_type(StreamType::SeekableFast)
        .unwrap();

    let player_clone = Arc::clone(&player);
    let seek_sender: Arc<Mutex<Option<IpcSender<bool>>>> = Arc::new(Mutex::new(None));
    let seek_sender_clone = seek_sender.clone();
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();
    let seek_data = Arc::new(AtomicBool::new(false));
    let seek_data_clone = seek_data.clone();
    let seek_to = Arc::new(AtomicUsize::new(0));
    let seek_to_clone = seek_to.clone();
    let t = thread::spawn(move || {
        let player = &player_clone;
        let mut buf_reader = BufReader::new(file);
        let mut buffer = [0; 1024];
        while !shutdown_clone.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(100));
            if seek_data_clone.load(Ordering::Relaxed) {
                seek_data_clone.store(false, Ordering::Relaxed);
                let offset = seek_to_clone.load(Ordering::Relaxed) as u64;
                if buf_reader.seek(SeekFrom::Start(offset)).is_err() {
                    eprintln!("BufReader - Could not seek to {:?}", offset);
                    break;
                }
                println!("BufReader - Seeked to {:?}", offset);
                seek_sender_clone.lock().unwrap().as_ref().unwrap().send(true);
            }
            match buf_reader.read(&mut buffer[..]) {
                Ok(0) => {
                    println!("Finished pushing data");
                    break;
                }
                Ok(size) => {
                    println!("Pushing data size {:?}", size);
                    player
                        .lock()
                        .unwrap()
                        .push_data(Vec::from(&buffer[0..size]))
                        .unwrap()
                },
                Err(e) => {
                    eprintln!("Error: {}", e);
                    break;
                }
            }
        }
    });

    player.lock().unwrap().play().unwrap();

    while let Ok(event) = receiver.recv() {
        match event {
            PlayerEvent::EndOfStream => {
                println!("EOF");
                break;
            }
            PlayerEvent::Error => {
                println!("Error");
                break;
            }
            PlayerEvent::MetadataUpdated(ref m) => {
                println!("Metadata updated! {:?}", m);
            }
            PlayerEvent::StateChanged(ref s) => {
                println!("Player state changed to {:?}", s);
            }
            PlayerEvent::FrameUpdated => eprint!("."),
            PlayerEvent::PositionChanged(p) => {
                println!("{:?}", p);
                if p == 1 {
                    println!("SEEKING");
                    player.lock().unwrap().seek(4., false).unwrap();
                }
            },
            PlayerEvent::SeekData(p, sender) => {
                println!("Seek requested to position {:?}", p);
                seek_data.store(true, Ordering::Relaxed);
                seek_to.store(p as usize, Ordering::Relaxed);
                *seek_sender.lock().unwrap() = Some(sender);
            },
            PlayerEvent::SeekDone(p) => {
                println!("Seeked to {:?}", p)
            },
        }
    }

    shutdown.store(true, Ordering::Relaxed);
    let _ = t.join();

    player.lock().unwrap().stop().unwrap();
}

fn main() {
    if let Ok(servo_media) = ServoMedia::get() {
        run_example(servo_media);
    }
}
