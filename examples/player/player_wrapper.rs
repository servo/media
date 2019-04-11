// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use ipc_channel::ipc;
use servo_media::player::frame::{Frame, FrameRenderer};
use servo_media::player::{GlContext, Player, PlayerEvent, StreamType};
use servo_media::ServoMedia;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::Builder;

pub struct PlayerWrapper {
    player: Arc<Mutex<Box<dyn Player>>>,
    shutdown: Arc<AtomicBool>,
    use_gl: bool,
}

impl PlayerWrapper {
    fn set_gl_params(
        player: &Arc<Mutex<Box<dyn Player>>>,
        windowed_context: &glutin::WindowedContext,
    ) -> Result<(), ()> {
        use glutin::os::ContextTraitExt;

        let context = windowed_context.context();
        let raw_handle = unsafe { context.raw_handle() };

        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        {
            use glutin::os::unix::RawHandle;

            match raw_handle {
                RawHandle::Egl(egl_context) => {
                    let gl_context = GlContext::Egl(egl_context as usize);
                    if let Some(gl_display) = unsafe { context.get_egl_display() } {
                        return player
                            .lock()
                            .unwrap()
                            .set_gl_params(gl_context, gl_display as usize);
                    }
                    Err(())
                }
                RawHandle::Glx(_) => Err(()),
            }
        }

        #[cfg(not(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        )))]
        {
            println!("GL rendering unavailable for this platform");
            Err(())
        }
    }

    pub fn new(path: &Path, windowed_context: Option<&glutin::WindowedContext>) -> Self {
        let servo_media = ServoMedia::get().unwrap();
        let player = Arc::new(Mutex::new(servo_media.create_player(StreamType::Seekable)));

        let use_gl = if let Some(windowed_context) = windowed_context {
            PlayerWrapper::set_gl_params(&player, windowed_context).is_ok()
        } else {
            false
        };

        let file = File::open(&path).unwrap();
        let metadata = file.metadata().unwrap();
        player
            .lock()
            .unwrap()
            .set_input_size(metadata.len())
            .unwrap();

        let (sender, receiver) = ipc::channel().unwrap();
        player.lock().unwrap().register_event_handler(sender);

        let player_ = player.clone();
        let player__ = player.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_ = shutdown.clone();
        let shutdown__ = shutdown.clone();

        let (seek_sender, seek_receiver) = mpsc::channel();

        Builder::new()
            .name("File reader".to_owned())
            .spawn(move || {
                let player = &player_;
                let shutdown = &shutdown_;

                let mut buf_reader = BufReader::new(file);
                let mut buffer = [0; 8192];
                let end_file = AtomicBool::new(false);

                while !shutdown.load(Ordering::Relaxed) {
                    if let Ok(offset) = seek_receiver.try_recv() {
                        if buf_reader.seek(SeekFrom::Start(offset)).is_err() {
                            eprintln!("BufReader - Could not seek to {:?}", offset);
                            break;
                        }
                        end_file.store(false, Ordering::Relaxed);
                    }

                    if !end_file.load(Ordering::Relaxed) {
                        match buf_reader.read(&mut buffer[..]) {
                            Ok(0) => {
                                println!("finished pushing data");
                                end_file.store(true, Ordering::Relaxed);
                            }
                            Ok(size) => {
                                if let Err(e) = player
                                    .lock()
                                    .unwrap()
                                    .push_data(Vec::from(&buffer[0..size]))
                                {
                                    println!("Can't push data: {:?}", e);
                                    //break;
                                }
                            }
                            Err(e) => {
                                eprintln!("Error: {}", e);
                                break;
                            }
                        }
                    }
                }

                println!("out loop");
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
                        PlayerEvent::Error(ref s) => {
                            println!("Player's Error {:?}", s);
                            break;
                        }
                        PlayerEvent::MetadataUpdated(ref m) => {
                            println!("Metadata updated! {:?}", m);
                        }
                        PlayerEvent::StateChanged(ref s) => {
                            println!("Player state changed to {:?}", s);
                        }
                        PlayerEvent::FrameUpdated => eprint!("."),
                        PlayerEvent::PositionChanged(_) => (),
                        PlayerEvent::SeekData(offset) => {
                            println!("Seek requested to position {:?}", offset);
                            seek_sender.send(offset).unwrap();
                        }
                        PlayerEvent::SeekDone(offset) => {
                            println!("Seek done to position {:?}", offset);
                        }
                        PlayerEvent::NeedData => {
                            println!("Player needs data");
                        }
                        PlayerEvent::EnoughData => {
                            println!("Player has enough data");
                        }
                    }

                    if shutdown.load(Ordering::Relaxed) {
                        break;
                    }
                }

                player.lock().unwrap().shutdown().unwrap();
                shutdown.store(true, Ordering::Relaxed);
            })
            .unwrap();

        player.lock().unwrap().play().unwrap();

        PlayerWrapper {
            player,
            shutdown,
            use_gl,
        }
    }

    pub fn shutdown(&self) {
        self.player.lock().unwrap().stop().unwrap();
        self.shutdown.store(true, Ordering::Relaxed);
    }

    pub fn use_gl(&self) -> bool {
        self.use_gl
    }

    pub fn register_frame_renderer(&self, renderer: Arc<Mutex<FrameRenderer>>) {
        self.player
            .lock()
            .unwrap()
            .register_frame_renderer(renderer);
    }
}
