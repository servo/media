use block::{Chunk, FRAMES_PER_BLOCK_USIZE};
use render_thread::AudioRenderThreadMsg;
use sink::AudioSink;
use std::cell::{Cell, RefCell};
use std::sync::mpsc::Sender;

pub struct ProcessedAudio(Box<[f32]>);

impl AsRef<[f32]> for ProcessedAudio {
    fn as_ref(&self) -> &[f32] {
        &self.0
    }
}

pub struct OfflineAudioContext {
    buffer: RefCell<Option<Vec<f32>>>,
    channel_count: usize,
    has_enough_data: Cell<bool>,
    length: usize,
    rendered_blocks: Cell<usize>,
    eos_callback: RefCell<Option<Box<Fn(Box<AsRef<[f32]>>) + Send + Sync + 'static>>>,
}

impl OfflineAudioContext {
    pub fn new(channel_count: usize, length: usize) -> Self {
        Self {
            buffer: RefCell::new(None),
            channel_count,
            has_enough_data: Cell::new(false),
            length,
            rendered_blocks: Cell::new(0),
            eos_callback: RefCell::new(None),
        }
    }
}

impl AudioSink for OfflineAudioContext {
    fn init(&self, _: f32, _: Sender<AudioRenderThreadMsg>) -> Result<(), ()> {
        Ok(())
    }

    fn play(&self) -> Result<(), ()> {
        self.has_enough_data.set(false);
        Ok(())
    }

    fn stop(&self) -> Result<(), ()> {
        self.has_enough_data.set(true);
        Ok(())
    }

    fn has_enough_data(&self) -> bool {
        self.has_enough_data.get()
            || (self.rendered_blocks.get() >= (self.length / FRAMES_PER_BLOCK_USIZE))
    }

    fn push_data(&self, chunk: Chunk) -> Result<(), ()> {
        {
            let offset = self.rendered_blocks.get() * FRAMES_PER_BLOCK_USIZE;
            let mut buffer = self.buffer.borrow_mut();
            if buffer.is_none() {
                *buffer = Some(vec![0.; self.channel_count * self.length]);
            }
            if let Some(ref mut buffer) = *buffer {
                for channel_number in 0..self.channel_count {
                    let channel_offset = offset + (channel_number * self.length);
                    let mut channel_data =
                        &mut buffer[channel_offset..channel_offset + FRAMES_PER_BLOCK_USIZE];
                    channel_data.copy_from_slice(chunk.blocks[0].data_chan(channel_number as u8));
                }
            };
            self.rendered_blocks.update(|blocks| blocks + 1);
        }

        if self.rendered_blocks.get() >= (self.length / FRAMES_PER_BLOCK_USIZE) {
            if let Some(callback) = self.eos_callback.borrow_mut().take() {
                let processed_audio =
                    ProcessedAudio(self.buffer.borrow_mut().take().unwrap().into_boxed_slice());
                callback(Box::new(processed_audio));
            }
        }

        Ok(())
    }

    fn set_eos_callback(&self, callback: Box<Fn(Box<AsRef<[f32]>>) + Send + Sync + 'static>) {
        *self.eos_callback.borrow_mut() = Some(callback);
    }
}
