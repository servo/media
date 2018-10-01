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

pub struct OfflineAudioSink {
    buffer: RefCell<Option<Vec<f32>>>,
    channel_count: usize,
    has_enough_data: Cell<bool>,
    length: usize,
    rendered_blocks: Cell<usize>,
    eos_callback: RefCell<Option<Box<Fn(Box<AsRef<[f32]>>) + Send + Sync + 'static>>>,
}

impl OfflineAudioSink {
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

// replace with ! when it stabilizes
#[derive(Debug)]
pub enum OfflineError {}

impl AudioSink for OfflineAudioSink {
    type Error = OfflineError;
    fn init(&self, _: f32, _: Sender<AudioRenderThreadMsg>) -> Result<(), OfflineError> {
        Ok(())
    }

    fn play(&self) -> Result<(), OfflineError> {
        self.has_enough_data.set(false);
        Ok(())
    }

    fn stop(&self) -> Result<(), OfflineError> {
        self.has_enough_data.set(true);
        Ok(())
    }

    fn has_enough_data(&self) -> bool {
        self.has_enough_data.get()
            || (self.rendered_blocks.get() * FRAMES_PER_BLOCK_USIZE >= self.length)
    }

    fn push_data(&self, mut chunk: Chunk) -> Result<(), OfflineError> {
        let offset = self.rendered_blocks.get() * FRAMES_PER_BLOCK_USIZE;
        let (last, copy_len) = if self.length - offset <= FRAMES_PER_BLOCK_USIZE {
            (true, self.length - offset)
        } else {
            (false, FRAMES_PER_BLOCK_USIZE)
        };
        let mut buffer = self.buffer.borrow_mut();
        if buffer.is_none() {
            *buffer = Some(vec![0.; self.channel_count * self.length]);
        }
        if chunk.len() == 0 {
            chunk.blocks.push(Default::default());
        }
        if chunk.blocks[0].is_empty() {
            chunk.blocks[0].explicit_silence();
        }
        if let Some(ref mut buffer) = *buffer {
            for channel_number in 0..self.channel_count {
                let channel_offset = offset + (channel_number * self.length);
                let mut channel_data = &mut buffer[channel_offset..channel_offset + copy_len];
                channel_data
                    .copy_from_slice(&chunk.blocks[0].data_chan(channel_number as u8)[0..copy_len]);
            }
        };
        self.rendered_blocks.set(self.rendered_blocks.get() + 1);

        if last {
            if let Some(callback) = self.eos_callback.borrow_mut().take() {
                let processed_audio = ProcessedAudio(buffer.take().unwrap().into_boxed_slice());
                callback(Box::new(processed_audio));
            }
        }

        Ok(())
    }

    fn set_eos_callback(&self, callback: Box<Fn(Box<AsRef<[f32]>>) + Send + Sync + 'static>) {
        *self.eos_callback.borrow_mut() = Some(callback);
    }
}
