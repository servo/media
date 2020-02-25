/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use failure::Error;
use ipc_channel::ipc::{self, IpcReceiver};
use servo_media::player;
use servo_media::player::video;
use servo_media::ServoMedia;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Debug, Fail)]
#[fail(display = "WebRender Error: {}", _0)]
struct WRError(std::string::String);

#[derive(Debug, Fail)]
#[fail(display = "Servo/Media Error: {}", _0)]
struct SMError(std::string::String);

#[derive(Debug, Fail)]
#[fail(display = "Error: {}", _0)]
struct MiscError(&'static str);

#[path = "renderer.rs"]
mod renderer;
use self::renderer::*;

#[path = "context.rs"]
mod context;
use self::context::*;

struct Notifier {
    events_proxy: winit::EventsLoopProxy,
}

impl Notifier {
    fn new(events_proxy: winit::EventsLoopProxy) -> Notifier {
        Notifier { events_proxy }
    }
}

impl webrender_api::RenderNotifier for Notifier {
    fn clone(&self) -> Box<dyn webrender_api::RenderNotifier> {
        Box::new(Notifier {
            events_proxy: self.events_proxy.clone(),
        })
    }

    fn wake_up(&self) {
        #[cfg(not(target_os = "android"))]
        let _ = self.events_proxy.wakeup();
    }

    fn new_frame_ready(
        &self,
        _: webrender_api::DocumentId,
        _scrolled: bool,
        _composite_needed: bool,
        _render_time: Option<u64>,
    ) {
        self.wake_up();
    }
}

enum PlayerCmd {
    Stop,
    Pause,
    Play,
    Seek(f64),
    Mute,
    None,
}

struct State {
    state: player::PlaybackState,
    pos: f64,
    duration: f64,
    mute: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            state: player::PlaybackState::Stopped,
            pos: 0.,
            duration: std::f64::NAN,
            mute: false,
        }
    }
}

pub struct Options {
    pub use_gl: bool,
    pub no_video: bool,
    pub wr_stats: bool,
}

pub struct App {
    events_loop: glutin::EventsLoop,
    windowed_context: glutin::WindowedContext<glutin::PossiblyCurrent>,
    webrender: webrender::Renderer,
    webrender_api: webrender_api::RenderApi,
    player: Arc<Mutex<dyn player::Player>>,
    file: File,
    player_event_receiver: IpcReceiver<player::PlayerEvent>,
    frame_renderer: Option<Arc<Mutex<MediaFrameRenderer>>>,
}

impl App {
    pub fn new(opts: Options, path: &Path) -> Result<App, Error> {
        // media file
        let file = File::open(&path)?;
        let metadata = file.metadata()?;

        // windowing
        let events_loop = glutin::EventsLoop::new();
        let window_builder = glutin::WindowBuilder::new()
            .with_dimensions(glutin::dpi::LogicalSize::new(1024 as _, 740 as _))
            .with_visibility(true);
        let windowed_context = glutin::ContextBuilder::new()
            .with_gl(glutin::GlRequest::GlThenGles {
                opengl_version: (3, 2),
                opengles_version: (3, 0),
            })
            .with_vsync(true)
            .build_windowed(window_builder, &events_loop)?;

        let windowed_context = unsafe { windowed_context.make_current().map_err(|(_, err)| err)? };

        let gl = match windowed_context.get_api() {
            glutin::Api::OpenGl => unsafe {
                gleam::gl::GlFns::load_with(|s| windowed_context.get_proc_address(s) as *const _)
            },
            glutin::Api::OpenGlEs => unsafe {
                gleam::gl::GlesFns::load_with(|s| windowed_context.get_proc_address(s) as *const _)
            },
            glutin::Api::WebGl => unreachable!("webgl is unsupported"),
        };

        println!("OpenGL version {}", gl.get_string(gleam::gl::VERSION));

        // WebRender
        let mut debug_flags = webrender::DebugFlags::empty();
        debug_flags.set(webrender::DebugFlags::PROFILER_DBG, opts.wr_stats);

        let (mut webrender, webrender_api_sender) = webrender::Renderer::new(
            gl,
            Box::new(Notifier::new(events_loop.create_proxy())),
            webrender::RendererOptions {
                resource_override_path: None,
                precache_flags: webrender::ShaderPrecacheFlags::empty(),
                device_pixel_ratio: windowed_context.window().get_hidpi_factor() as _,
                clear_color: Some(webrender_api::ColorF::BLACK),
                debug_flags,
                ..Default::default()
            },
            None,
        )
        .map_err(|err| WRError(format!("{:?}", err)))?;

        let webrender_api = webrender_api_sender.create_api();

        // player
        let (player_event_sender, player_event_receiver) = ipc::channel::<player::PlayerEvent>()?;
        let servo_media =
            ServoMedia::get().map_err(|_| MiscError("Failed to get media backend"))?;

        let frame_renderer = if !opts.no_video {
            Some(Arc::new(Mutex::new(MediaFrameRenderer::new(
                webrender_api_sender,
            ))))
        } else {
            None
        };
        let renderer: Option<Arc<Mutex<dyn video::VideoFrameRenderer>>> = match frame_renderer
            .clone()
        {
            None => None,
            Some(renderer) => {
                webrender
                    .set_external_image_handler(Box::new(MediaFrameHandler::new(renderer.clone())));
                Some(renderer)
            }
        };

        let player = servo_media.create_player(
            &servo_media::ClientContextId::build(1, 1),
            player::StreamType::Seekable,
            player_event_sender,
            renderer,
            None,
            Box::new(PlayerContextGlutin::new(opts.use_gl, &windowed_context)),
        );

        player
            .lock()
            .unwrap()
            .set_input_size(metadata.len())
            .map_err(|_| MiscError("Failed to set media input size"))?;

        Ok(App {
            events_loop,
            windowed_context,
            webrender,
            webrender_api,
            player,
            file,
            player_event_receiver,
            frame_renderer,
        })
    }

    fn into_context(self) -> glutin::WindowedContext<glutin::PossiblyCurrent> {
        self.webrender.deinit();
        self.windowed_context
    }
}

pub fn main_loop(mut app: App) -> Result<glutin::WindowedContext<glutin::PossiblyCurrent>, Error> {
    let windowed_context = &mut app.windowed_context;
    let events_loop = &mut app.events_loop;
    let webrender = &mut app.webrender;
    let receiver = &mut app.player_event_receiver;
    let file = &mut app.file;
    let player = &mut app.player;

    let device_pixel_ratio = windowed_context.window().get_hidpi_factor();
    let window_size = windowed_context
        .window()
        .get_inner_size()
        .ok_or_else(|| MiscError("Failed to get window inner size"))?;
    let mut framebuffer_size = {
        let glutin::dpi::PhysicalSize { width, height } =
            window_size.to_physical(device_pixel_ratio as f64);
        webrender_api::DeviceIntSize::new(width as i32, height as i32)
    };
    let webrender_document = app.webrender_api.add_document(framebuffer_size, 0);

    let epoch = webrender_api::Epoch(0);
    let webrender_pipeline = webrender_api::PipelineId(0, 0);
    let layout_size =
        framebuffer_size.to_f32() / euclid::TypedScale::new(device_pixel_ratio as f32);

    // first frame ?
    {
        let mut transaction = webrender_api::Transaction::new();
        let builder = webrender_api::DisplayListBuilder::new(webrender_pipeline, layout_size);
        transaction.set_display_list(epoch, None, layout_size, builder.finalize(), true);
        transaction.set_root_pipeline(webrender_pipeline);
        transaction.generate_frame();
        app.webrender_api
            .send_transaction(webrender_document, transaction);
    }

    // file reader
    let mut buf_reader = BufReader::new(file);
    let mut buffer = [0; 16384];

    player
        .lock()
        .unwrap()
        .play()
        .map_err(|_| MiscError("Failed to start player"))?;

    let mut running = true;
    let mut input_eos = false;
    let mut playercmd = PlayerCmd::None;
    let mut frameupdated = false;
    let mut playerstate: State = Default::default();

    while running {
        events_loop.poll_events(|event| match event {
            glutin::Event::WindowEvent { event, .. } => match event {
                glutin::WindowEvent::CloseRequested => playercmd = PlayerCmd::Stop,
                glutin::WindowEvent::Resized(logical_size) => {
                    let size = logical_size.to_physical(device_pixel_ratio);
                    windowed_context.resize(size);

                    framebuffer_size =
                        webrender_api::DeviceIntSize::new(size.width as i32, size.height as i32);
                }
                glutin::WindowEvent::KeyboardInput {
                    input:
                        glutin::KeyboardInput {
                            state: glutin::ElementState::Pressed,
                            virtual_keycode: Some(key),
                            ..
                        },
                    ..
                } => match key {
                    glutin::VirtualKeyCode::Escape | glutin::VirtualKeyCode::Q => {
                        playercmd = PlayerCmd::Stop
                    }
                    glutin::VirtualKeyCode::Right => playercmd = PlayerCmd::Seek(10.),
                    glutin::VirtualKeyCode::Left => playercmd = PlayerCmd::Seek(-10.),
                    glutin::VirtualKeyCode::Space => {
                        playercmd = match playerstate.state {
                            player::PlaybackState::Paused => PlayerCmd::Play,
                            player::PlaybackState::Playing | player::PlaybackState::Buffering => {
                                PlayerCmd::Pause
                            }
                            _ => PlayerCmd::None,
                        };
                    }
                    glutin::VirtualKeyCode::M => playercmd = PlayerCmd::Mute,
                    _ => (),
                },
                _ => (), //println!("glutin event: {:?}", event),
            },
            _ => (), // not our window
        });

        match playercmd {
            PlayerCmd::Stop => {
                player
                    .lock()
                    .unwrap()
                    .stop()
                    .map_err(|_| MiscError("Failed to stop player"))?;
                input_eos = true;
            }
            PlayerCmd::Pause => {
                player
                    .lock()
                    .unwrap()
                    .pause()
                    .map_err(|_| MiscError("Failed to pause player"))?;
            }
            PlayerCmd::Play => {
                player
                    .lock()
                    .unwrap()
                    .play()
                    .map_err(|_| MiscError("Failed to start player"))?;
            }
            PlayerCmd::Seek(time) => {
                let time = playerstate.pos + time;
                let time = f64::min(time, playerstate.duration);
                let time = f64::max(time, 0.);

                eprintln!("Seeking {}", time);
                player
                    .lock()
                    .unwrap()
                    .seek(time)
                    .map_err(|_| MiscError("Failed to seek"))?;
            }
            PlayerCmd::Mute => {
                playerstate.mute = !playerstate.mute;
                player
                    .lock()
                    .unwrap()
                    .mute(playerstate.mute)
                    .map_err(|_| MiscError("Failed to mute player"))?;
            }
            _ => (),
        }
        playercmd = PlayerCmd::None;

        while let Ok(event) = receiver.try_recv() {
            match event {
                player::PlayerEvent::EndOfStream => running = false,
                player::PlayerEvent::Error(ref s) => Err(SMError(format!("{:?}", s)))?,
                player::PlayerEvent::MetadataUpdated(metadata) => {
                    println!("{:?}", metadata);
                    if let Some(duration) = metadata.duration {
                        playerstate.duration = duration.as_secs() as f64;
                    } else {
                        playerstate.duration = std::f64::INFINITY;
                    }
                }
                player::PlayerEvent::StateChanged(state) => {
                    println!("Player state changed to {:?}", state);
                    playerstate.state = state;
                    match playerstate.state {
                        player::PlaybackState::Stopped => running = false,
                        _ => (),
                    }
                }
                player::PlayerEvent::SeekData(offset, seek_lock) => {
                    input_eos = false;
                    seek_lock.unlock(if let Ok(pos) = buf_reader.seek(SeekFrom::Start(offset)) {
                        offset == pos
                    } else {
                        false
                    });
                }
                player::PlayerEvent::NeedData => {
                    if !input_eos {
                        let bytes_read = buf_reader.read(&mut buffer[..])?;
                        if bytes_read == 0 {
                            player
                                .lock()
                                .unwrap()
                                .end_of_stream()
                                .and_then(|_| {
                                    input_eos = true;
                                    Ok(())
                                })
                                .map_err(|_| SMError("Error at setting EOS".to_string()))?;
                        } else {
                            player
                                .lock()
                                .unwrap()
                                .push_data(Vec::from(&buffer[0..bytes_read]))
                                .or_else(|err| {
                                    if err == player::PlayerError::EnoughData {
                                        Ok(())
                                    } else {
                                        Err(SMError(format!("Error at pushing data: {:?}", err)))
                                    }
                                })?;
                        }
                    }
                }
                player::PlayerEvent::VideoFrameUpdated => frameupdated = true,
                player::PlayerEvent::PositionChanged(pos) => playerstate.pos = pos as f64,
                _ => (),
            }
        }

        if frameupdated {
            let mut builder =
                webrender_api::DisplayListBuilder::new(webrender_pipeline, layout_size);
            let mut transaction = webrender_api::Transaction::new();

            {
                app.frame_renderer.as_ref().and_then(|renderer| {
                    renderer.lock().unwrap().current_frame().and_then(|frame| {
                        let content_bounds = webrender_api::LayoutRect::new(
                            webrender_api::LayoutPoint::zero(),
                            webrender_api::LayoutSize::new(frame.1 as f32, frame.2 as f32),
                        );
                        let info = webrender_api::LayoutPrimitiveInfo::new(content_bounds);
                        let root_space_and_clip =
                            webrender_api::SpaceAndClipInfo::root_scroll(webrender_pipeline);

                        builder.push_image(
                            &info,
                            &root_space_and_clip,
                            info.rect.size,
                            webrender_api::LayoutSize::zero(),
                            webrender_api::ImageRendering::Auto,
                            webrender_api::AlphaType::PremultipliedAlpha,
                            frame.0.clone(),
                            webrender_api::ColorF::WHITE,
                        );

                        Some(frame)
                    })
                });
            }

            transaction.set_display_list(epoch, None, layout_size, builder.finalize(), true);
            transaction.generate_frame();
            app.webrender_api
                .send_transaction(webrender_document, transaction);

            frameupdated = false;
        }

        webrender.update();
        webrender
            .render(framebuffer_size)
            .map_err(|err| WRError(format!("{:?}", err)))?;
        let _ = webrender.flush_pipeline_info();
        windowed_context.swap_buffers()?;
    }

    Ok(app.into_context())
}

pub fn cleanup(
    _windowed_context: glutin::WindowedContext<glutin::PossiblyCurrent>,
) -> Result<(), failure::Error> {
    Ok(())
}
