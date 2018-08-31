// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

#![feature(rustc_private)]

extern crate gleam;
extern crate glutin;
extern crate ipc_channel;
extern crate servo_media;
extern crate time;
extern crate webrender;
extern crate winit;

use gleam::gl;
use ipc_channel::ipc;
use servo_media::player::frame::{Frame, FrameRenderer};
use servo_media::player::{Player, PlayerEvent};
use servo_media::ServoMedia;
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::Builder;
use ui::HandyDandyRectBuilder;
use webrender::api::*;

#[path = "ui.rs"]
mod ui;

struct PlayerWrapper {
    player: Arc<Mutex<Box<Player>>>,
    shutdown: Arc<AtomicBool>,
}

impl PlayerWrapper {
    pub fn new(path: &Path) -> Self {
        let servo_media = ServoMedia::get().unwrap();
        let player = Arc::new(Mutex::new(servo_media.create_player().unwrap()));
        let file = File::open(&path).unwrap();
        let metadata = file.metadata().unwrap();
        player.lock().unwrap().set_input_size(metadata.len());
        let (sender, receiver) = ipc::channel().unwrap();
        player.lock().unwrap().register_event_handler(sender);
        player
            .lock()
            .unwrap()
            .setup()
            .expect("couldn't setup player");
        let player_ = player.clone();
        let player__ = player.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_ = shutdown.clone();
        let shutdown__ = shutdown.clone();
        Builder::new()
            .name("File reader".to_owned())
            .spawn(move || {
                let player = &player_;
                let shutdown = &shutdown_;
                let mut buf_reader = BufReader::new(file);
                let mut buffer = [0; 8192];
                while !shutdown.load(Ordering::Relaxed) {
                    match buf_reader.read(&mut buffer[..]) {
                        Ok(0) => {
                            println!("finished pushing data");
                            break;
                        }
                        Ok(size) => {
                            if let Err(_) = player
                                .lock()
                                .unwrap()
                                .push_data(Vec::from(&buffer[0..size]))
                            {
                                break;
                            }
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            break;
                        }
                    }
                }
            })
            .unwrap();

        Builder::new()
            .name("Player event loop".to_owned())
            .spawn(move || {
                let player = &player__;
                let shutdown = &shutdown__;
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
                    }
                }
                player.lock().unwrap().stop();
                shutdown.store(true, Ordering::Relaxed);
            })
            .unwrap();

        player.lock().unwrap().play();

        PlayerWrapper { player, shutdown }
    }

    fn shutdown(&self) {
        self.player.lock().unwrap().stop();
        self.shutdown.store(true, Ordering::Relaxed);
    }

    fn register_frame_renderer(&self, renderer: Arc<Mutex<FrameRenderer>>) {
        self.player
            .lock()
            .unwrap()
            .register_frame_renderer(renderer);
    }
}

struct App {
    frame_queue: Mutex<Vec<Frame>>,
    current_frame: Mutex<Option<Frame>>,
}

impl App {
    fn new() -> Self {
        Self {
            frame_queue: Mutex::new(Vec::new()),
            current_frame: Mutex::new(None),
        }
    }
}

impl ui::Example for App {
    fn render(
        &self,
        api: &RenderApi,
        builder: &mut DisplayListBuilder,
        txn: &mut Transaction,
        _framebuffer_size: DeviceUintSize,
        _pipeline_id: PipelineId,
        _document_id: DocumentId,
    ) {
        let frame = if self.frame_queue.lock().unwrap().is_empty() {
            let mut frame = self.current_frame.lock().unwrap();
            if frame.is_none() {
                return;
            }
            frame.take().unwrap()
        } else {
            self.frame_queue.lock().unwrap().pop().unwrap()
        };
        let width = frame.get_width() as u32;
        let height = frame.get_height() as u32;
        let image_descriptor =
            ImageDescriptor::new(width, height, ImageFormat::BGRA8, false, false);
        let image_data = ImageData::new_shared(frame.get_data().clone());
        *self.current_frame.lock().unwrap() = Some(frame);
        let image_key = api.generate_image_key();
        txn.add_image(image_key, image_descriptor, image_data, None);
        let bounds = (0, 0).to(width as i32, height as i32);
        let info = LayoutPrimitiveInfo::new(bounds);
        builder.push_stacking_context(
            &info,
            None,
            TransformStyle::Flat,
            MixBlendMode::Normal,
            Vec::new(),
            GlyphRasterSpace::Screen,
        );
        let image_size = LayoutSize::new(width as f32, height as f32);
        let info = LayoutPrimitiveInfo::new(bounds);
        builder.push_image(
            &info,
            image_size,
            LayoutSize::zero(),
            ImageRendering::Auto,
            AlphaType::PremultipliedAlpha,
            image_key,
        );
        builder.pop_stacking_context();
    }

    fn on_event(&self, _: winit::WindowEvent, _: &RenderApi, _: DocumentId) -> bool {
        false
    }

    fn needs_repaint(&self) -> bool {
        !self.frame_queue.lock().unwrap().is_empty()
    }

    fn get_image_handlers(
        &self,
        _gl: &gl::Gl,
    ) -> (
        Option<Box<webrender::ExternalImageHandler>>,
        Option<Box<webrender::OutputImageHandler>>,
    ) {
        (None, None)
    }

    fn draw_custom(&self, _gl: &gl::Gl) {}
}

impl FrameRenderer for App {
    fn render(&self, frame: Frame) {
        self.frame_queue.lock().unwrap().push(frame);
    }
}

fn main() {
    let args: Vec<_> = env::args().collect();
    let filename: &str = if args.len() == 2 {
        args[1].as_ref()
    } else {
        panic!("Usage: cargo run --bin player <file_path>")
    };

    let path = Path::new(filename);
    let player_wrapper = PlayerWrapper::new(&path);
    let app = Arc::new(Mutex::new(App::new()));
    player_wrapper.register_frame_renderer(app.clone());
    ui::main_wrapper(app, None);
    player_wrapper.shutdown();
}
