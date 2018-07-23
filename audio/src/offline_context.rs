use block::{Chunk, FRAMES_PER_BLOCK_USIZE};
use render_thread::AudioRenderThreadMsg;
use sink::AudioSink;
use std::cell::{Cell, RefCell};
use std::sync::mpsc::Sender;

pub struct OfflineAudioContext {
    buffers: RefCell<Vec<Vec<f32>>>,
    has_enough_data: Cell<bool>,
    length: usize,
    rendered_blocks: Cell<usize>,
}

impl OfflineAudioContext {
    pub fn new(number_of_channels: u8, length: usize) -> Self {
        let buffers = vec![Vec::with_capacity(length * FRAMES_PER_BLOCK_USIZE); number_of_channels as usize];
        Self {
            buffers: RefCell::new(buffers),
            has_enough_data: Cell::new(false),
            length,
            rendered_blocks: Cell::new(0),
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
        self.has_enough_data.get() || (self.rendered_blocks.get() >= (self.length / FRAMES_PER_BLOCK_USIZE))
    }

    fn push_data(&self, chunk: Chunk) -> Result<(), ()> {
        let mut buffers = self.buffers.borrow_mut();
        let channel_count = buffers.len();
        for chan in 0..channel_count {
            buffers[chan].extend_from_slice(chunk.blocks[0].data_chan(chan as u8));
        } 

        self.rendered_blocks.update(|blocks| blocks + 1);

        Ok(())
    }
}
