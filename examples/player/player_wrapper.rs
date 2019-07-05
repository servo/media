// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use ipc_channel::ipc;
use servo_media::player::context::PlayerGLContext;
use servo_media::player::frame::{Frame, FrameRenderer};
use servo_media::player::{Player, PlayerError, PlayerEvent, StreamType};
use servo_media::{ClientContextId, ServoMedia};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::Builder;

pub struct PlayerWrapper {
    player: Arc<Mutex<dyn Player>>,
    shutdown: Arc<AtomicBool>,
    client_context_id: ClientContextId,
}

impl PlayerWrapper {
    pub fn new(
        id: &ClientContextId,
        path: &Path,
        renderer: Option<Arc<Mutex<dyn FrameRenderer>>>,
        gl_context: Box<dyn PlayerGLContext>,
    ) -> Self {
        let (sender, receiver) = ipc::channel().unwrap();
        let servo_media = ServoMedia::get().unwrap();
        let player =
            servo_media.create_player(id, StreamType::Seekable, sender, renderer, gl_context);

        let file = File::open(&path).unwrap();
        let metadata = file.metadata().unwrap();
        player
            .lock()
            .unwrap()
            .set_input_size(metadata.len())
            .unwrap();

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
                                match player
                                    .lock()
                                    .unwrap()
                                    .push_data(Vec::from(&buffer[0..size]))
                                {
                                    Ok(_) => (),
                                    Err(PlayerError::EnoughData) => {
                                        print!("!");
                                    }
                                    Err(e) => {
                                        println!("Can't push data: {:?}", e);
                                        break;
                                    }
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
            client_context_id: *id,
        }
    }

    pub fn shutdown(&self) {
        ServoMedia::get()
            .unwrap()
            .shutdown_player(&self.client_context_id, self.player.clone());
        self.shutdown.store(true, Ordering::Relaxed);
    }

    pub fn use_gl(&self) -> bool {
        self.player.lock().unwrap().render_use_gl()
    }
}
