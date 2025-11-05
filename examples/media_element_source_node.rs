extern crate ipc_channel;
extern crate servo_media;
extern crate servo_media_auto;

use ipc_channel::ipc;
use servo_media::audio::media_element_source_node::MediaElementSourceNodeMessage;
use servo_media::audio::node::{AudioNodeInit, AudioNodeMessage};
use servo_media::audio::panner_node::PannerNodeOptions;
use servo_media::audio::param::{ParamDir, ParamType, RampKind, UserAutomationEvent};
use servo_media::player::context::{GlApi, GlContext, NativeDisplay, PlayerGLContext};
use servo_media::player::{PlayerEvent, StreamType};
use servo_media::{ClientContextId, ServoMedia};
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
    let context = servo_media
        .create_audio_context(&ClientContextId::build(1, 1), Default::default())
        .unwrap();
    let context = context.lock().unwrap();
    let listener = context.listener();

    let source_node =
        context.create_node(AudioNodeInit::MediaElementSourceNode, Default::default());

    let (sender, receiver) = mpsc::channel();
    context.message_node(
        source_node,
        AudioNodeMessage::MediaElementSourceNode(MediaElementSourceNodeMessage::GetAudioRenderer(
            sender,
        )),
    );
    let audio_renderer = receiver.recv().unwrap();

    let mut options = PannerNodeOptions::default();
    options.cone_outer_angle = 0.;
    options.position_x = 100.;
    options.position_y = 0.;
    options.position_z = 100.;
    options.ref_distance = 100.;
    options.rolloff_factor = 0.01;
    let panner = context.create_node(AudioNodeInit::PannerNode(options), Default::default());

    let dest = context.dest_node();

    context.connect_ports(source_node.output(0), panner.input(0));
    context.connect_ports(panner.output(0), dest.input(0));

    let (sender, receiver) = ipc::channel().unwrap();
    let player = servo_media.create_player(
        &ClientContextId::build(1, 1),
        StreamType::Seekable,
        sender,
        None,
        Some(audio_renderer),
        Box::new(PlayerContextDummy()),
    );

    let filename = "./examples/resources/viper_cut.ogg";
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

    let _ = context.resume();

    // trace a square around your head twice
    context.message_node(
        panner,
        AudioNodeMessage::SetParam(
            ParamType::Position(ParamDir::X),
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, -100., 0.2),
        ),
    );
    context.message_node(
        panner,
        AudioNodeMessage::SetParam(
            ParamType::Position(ParamDir::Z),
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 100., 0.2),
        ),
    );
    context.message_node(
        panner,
        AudioNodeMessage::SetParam(
            ParamType::Position(ParamDir::X),
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, -100., 0.4),
        ),
    );
    context.message_node(
        panner,
        AudioNodeMessage::SetParam(
            ParamType::Position(ParamDir::Z),
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, -100., 0.4),
        ),
    );
    context.message_node(
        panner,
        AudioNodeMessage::SetParam(
            ParamType::Position(ParamDir::X),
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 100., 0.6),
        ),
    );
    context.message_node(
        panner,
        AudioNodeMessage::SetParam(
            ParamType::Position(ParamDir::Z),
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, -100., 0.6),
        ),
    );
    context.message_node(
        panner,
        AudioNodeMessage::SetParam(
            ParamType::Position(ParamDir::X),
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 100., 0.8),
        ),
    );
    context.message_node(
        panner,
        AudioNodeMessage::SetParam(
            ParamType::Position(ParamDir::Z),
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 100., 0.8),
        ),
    );

    context.message_node(
        panner,
        AudioNodeMessage::SetParam(
            ParamType::Position(ParamDir::X),
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, -100., 1.0),
        ),
    );
    context.message_node(
        panner,
        AudioNodeMessage::SetParam(
            ParamType::Position(ParamDir::Z),
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 100., 1.0),
        ),
    );
    context.message_node(
        panner,
        AudioNodeMessage::SetParam(
            ParamType::Position(ParamDir::X),
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, -100., 1.2),
        ),
    );
    context.message_node(
        panner,
        AudioNodeMessage::SetParam(
            ParamType::Position(ParamDir::Z),
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, -100., 1.2),
        ),
    );
    context.message_node(
        panner,
        AudioNodeMessage::SetParam(
            ParamType::Position(ParamDir::X),
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 100., 1.4),
        ),
    );
    context.message_node(
        panner,
        AudioNodeMessage::SetParam(
            ParamType::Position(ParamDir::Z),
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, -100., 1.4),
        ),
    );
    context.message_node(
        panner,
        AudioNodeMessage::SetParam(
            ParamType::Position(ParamDir::X),
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 100., 1.6),
        ),
    );
    context.message_node(
        panner,
        AudioNodeMessage::SetParam(
            ParamType::Position(ParamDir::Z),
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 100., 1.6),
        ),
    );
    // now it runs away
    context.message_node(
        panner,
        AudioNodeMessage::SetParam(
            ParamType::Position(ParamDir::Z),
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 10000., 3.),
        ),
    );
    context.message_node(
        listener,
        AudioNodeMessage::SetParam(
            ParamType::Position(ParamDir::Z),
            UserAutomationEvent::SetValueAtTime(0., 3.),
        ),
    );
    // chase it
    context.message_node(
        listener,
        AudioNodeMessage::SetParam(
            ParamType::Position(ParamDir::Z),
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, 10000., 4.),
        ),
    );

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
            PlayerEvent::StateChanged(ref s) => {
                println!("\nPlayer state changed to {:?}", s);
            },
            PlayerEvent::VideoFrameUpdated => {},
            PlayerEvent::PositionChanged(_) => println!("."),
            PlayerEvent::SeekData(p, seek_lock) => {
                println!("\nSeek requested to position {:?}", p);
                seek_sender.send(p).unwrap();
                seek_lock.unlock(true);
            },
            PlayerEvent::SeekDone(p) => println!("\nSeeked to {:?}", p),
            PlayerEvent::NeedData => println!("\nNeedData"),
            PlayerEvent::EnoughData => println!("\nEnoughData"),
        }
    }

    shutdown.store(true, Ordering::Relaxed);
    let _ = context.close();
    let _ = t.join();
}

fn main() {
    ServoMedia::init::<servo_media_auto::Backend>();
    let servo_media = ServoMedia::get();
    run_example(servo_media);
}
