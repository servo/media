extern crate ipc_channel;
extern crate servo_media;
extern crate servo_media_auto;

use ipc_channel::ipc;
use servo_media::player::context::{GlApi, GlContext, NativeDisplay, PlayerGLContext};
use servo_media::player::{PlayerEvent, StreamType};
use servo_media::{ClientContextId, ServoMedia};
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
    let (sender, receiver) = ipc::channel().unwrap();

    let player = servo_media.create_player(
        &ClientContextId::build(1, 1),
        StreamType::Stream,
        sender,
        None,
        None,
        Box::new(PlayerContextDummy()),
    );

    let audio_stream = servo_media.create_audiostream();
    player
        .lock()
        .unwrap()
        .set_stream(&audio_stream, /* only stream */ true)
        .unwrap();

    player.lock().unwrap().play().unwrap();

    while let Ok(event) = receiver.recv() {
        match event {
            PlayerEvent::EndOfStream => {
                println!("\nEOF");
                break;
            }
            PlayerEvent::Error(ref s) => {
                println!("\nError: {:?}", s);
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
                if p == 4 {
                    break;
                }
                println!("Position changed {:?}", p)
            }
            PlayerEvent::SeekData(_, _) => {
                println!("\nERROR: Should not receive SeekData for streams")
            }
            PlayerEvent::SeekDone(_) => {
                println!("\nERROR: Should not receive SeekDone for streams")
            }
            PlayerEvent::NeedData => println!("\nERROR: Should not receive NeedData for streams"),
            PlayerEvent::EnoughData => {
                println!("\nERROR: Should not receive EnoughData for streams")
            }
        }
    }
}

fn main() {
    ServoMedia::init::<servo_media_auto::Backend>();
    let servo_media = ServoMedia::get();
    run_example(servo_media);
}
