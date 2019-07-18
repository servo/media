// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use servo_media::player::frame;
use std::mem;
use std::sync::{Arc, Mutex};

#[derive(PartialEq)]
enum FrameStatus {
    Locked,
    Unlocked,
}

struct FrameHolder(FrameStatus, frame::Frame);

impl FrameHolder {
    fn new(frame: frame::Frame) -> FrameHolder {
        FrameHolder(FrameStatus::Unlocked, frame)
    }

    fn lock(&mut self) {
        if self.0 == FrameStatus::Unlocked {
            self.0 = FrameStatus::Locked;
        };
    }

    fn unlock(&mut self) {
        if self.0 == FrameStatus::Locked {
            self.0 = FrameStatus::Unlocked;
        };
    }

    fn set(&mut self, new_frame: frame::Frame) {
        if self.0 == FrameStatus::Unlocked {
            self.1 = new_frame
        };
    }

    fn get(&self) -> (u32, euclid::Size2D<i32>, usize) {
        if self.0 == FrameStatus::Locked {
            (
                self.1.get_texture_id(),
                euclid::Size2D::new(self.1.get_width(), self.1.get_height()),
                0,
            )
        } else {
            unreachable!();
        }
    }
}

pub struct MediaFrameRenderer {
    webrender_api: webrender_api::RenderApi,
    current_frame: Option<(webrender_api::ImageKey, i32, i32)>,
    old_frame: Option<webrender_api::ImageKey>,
    very_old_frame: Option<webrender_api::ImageKey>,
    current_frame_holder: Option<FrameHolder>,
}

impl MediaFrameRenderer {
    pub fn new(webrender_api_sender: webrender_api::RenderApiSender) -> Self {
        Self {
            webrender_api: webrender_api_sender.create_api(),
            current_frame: None,
            old_frame: None,
            very_old_frame: None,
            current_frame_holder: None,
        }
    }

    pub fn current_frame(&self) -> Option<(webrender_api::ImageKey, i32, i32)> {
        self.current_frame
    }

    fn lock(&mut self) -> (u32, euclid::Size2D<i32>, usize) {
        self.current_frame_holder
            .as_mut()
            .map(|holder| {
                holder.lock();
                holder.get()
            })
            .unwrap()
    }

    fn unlock(&mut self) {
        self.current_frame_holder
            .as_mut()
            .map(|holder| holder.unlock());
    }
}

impl frame::FrameRenderer for MediaFrameRenderer {
    fn render(&mut self, frame: frame::Frame) {
        let mut transaction = webrender_api::Transaction::new();

        if let Some(old_image_key) = mem::replace(&mut self.very_old_frame, self.old_frame.take()) {
            transaction.delete_image(old_image_key);
        }

        let descriptor = webrender_api::ImageDescriptor::new(
            frame.get_width(),
            frame.get_height(),
            webrender_api::ImageFormat::BGRA8,
            false,
            false,
        );

        match self.current_frame {
            Some((ref image_key, ref width, ref height))
                if *width == frame.get_width() && *height == frame.get_height() =>
            {
                if !frame.is_gl_texture() {
                    transaction.update_image(
                        *image_key,
                        descriptor,
                        webrender_api::ImageData::Raw(frame.get_data()),
                        &webrender_api::DirtyRect::All,
                    );
                } else {
                    self.current_frame_holder
                        .get_or_insert_with(|| FrameHolder::new(frame.clone()))
                        .set(frame);
                }

                if let Some(old_image_key) = self.old_frame.take() {
                    transaction.delete_image(old_image_key);
                }
            }
            Some((ref mut image_key, ref mut width, ref mut height)) => {
                self.old_frame = Some(*image_key);

                let new_image_key = self.webrender_api.generate_image_key();

                /* update current_frame */
                *image_key = new_image_key;
                *width = frame.get_width();
                *height = frame.get_height();

                let image_data = if frame.is_gl_texture() {
                    self.current_frame_holder
                        .get_or_insert_with(|| FrameHolder::new(frame.clone()))
                        .set(frame);

                    webrender_api::ImageData::External(webrender_api::ExternalImageData {
                        id: webrender_api::ExternalImageId(0),
                        channel_index: 0,
                        image_type: webrender_api::ExternalImageType::TextureHandle(
                            webrender_api::TextureTarget::Default,
                        ),
                    })
                } else {
                    webrender_api::ImageData::Raw(frame.get_data())
                };
                transaction.add_image(new_image_key, descriptor, image_data, None);
            }
            None => {
                let image_key = self.webrender_api.generate_image_key();
                self.current_frame = Some((image_key, frame.get_width(), frame.get_height()));

                let image_data = if frame.is_gl_texture() {
                    self.current_frame_holder = Some(FrameHolder::new(frame));

                    webrender_api::ImageData::External(webrender_api::ExternalImageData {
                        id: webrender_api::ExternalImageId(0),
                        channel_index: 0,
                        image_type: webrender_api::ExternalImageType::TextureHandle(
                            webrender_api::TextureTarget::Default,
                        ),
                    })
                } else {
                    webrender_api::ImageData::Raw(frame.get_data())
                };
                transaction.add_image(image_key, descriptor, image_data, None);
            }
        }

        self.webrender_api
            .update_resources(transaction.resource_updates);
    }
}

pub struct MediaFrameHandler {
    renderer: Arc<Mutex<MediaFrameRenderer>>,
}

impl MediaFrameHandler {
    pub fn new(renderer: Arc<Mutex<MediaFrameRenderer>>) -> Self {
        Self { renderer }
    }
}

impl webrender::ExternalImageHandler for MediaFrameHandler {
    fn lock(
        &mut self,
        _key: webrender_api::ExternalImageId,
        _channel_index: u8,
        _rendering: webrender_api::ImageRendering,
    ) -> webrender::ExternalImage {
        let (texture_id, size, _) = self.renderer.lock().unwrap().lock();
        webrender::ExternalImage {
            uv: webrender_api::TexelRect::new(0., 0., size.width as f32, size.height as f32),
            source: webrender::ExternalImageSource::NativeTexture(texture_id),
        }
    }

    fn unlock(&mut self, _key: webrender_api::ExternalImageId, _channel_index: u8) {
        self.renderer.lock().unwrap().unlock()
    }
}
