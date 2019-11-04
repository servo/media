use block::{Block, Chunk, FRAMES_PER_BLOCK};
use node::{AudioNodeEngine, AudioNodeType, BlockInfo, ChannelInfo};
use player::audio::AudioRenderer;
use player::Player;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct MediaElementSourceNodeOptions {
    pub player: Arc<Mutex<dyn Player>>,
}

#[derive(AudioNodeCommon)]
pub(crate) struct MediaElementSourceNode {
    channel_info: ChannelInfo,
    buffers: Arc<Mutex<Vec<Vec<f32>>>>,
    playback_offset: usize,
}

impl MediaElementSourceNode {
    pub fn new(options: MediaElementSourceNodeOptions, channel_info: ChannelInfo) -> Self {
        let buffers = Arc::new(Mutex::new(Vec::new()));
        let renderer = Arc::new(Mutex::new(MediaElementSourceNodeRenderer::new(
            buffers.clone(),
        )));
        let _ = options.player.lock().unwrap().set_audio_renderer(renderer);
        Self {
            channel_info,
            buffers,
            playback_offset: 0,
        }
    }
}

impl AudioNodeEngine for MediaElementSourceNode {
    fn node_type(&self) -> AudioNodeType {
        AudioNodeType::MediaElementSourceNode
    }

    fn process(&mut self, mut inputs: Chunk, info: &BlockInfo) -> Chunk {
        debug_assert!(inputs.len() == 0);

        inputs.blocks.push(Default::default());

        let buffers = self.buffers.lock().unwrap();
        let chans = buffers.len() as u8;

        if chans == 0 {
            return inputs;
        }

        let len = buffers[0].len();

        let frames_per_block = FRAMES_PER_BLOCK.0 as usize;
        let samples_to_copy = if self.playback_offset + frames_per_block > len {
            len - self.playback_offset
        } else {
            frames_per_block
        };
        let next_offset = self.playback_offset + samples_to_copy;
        if samples_to_copy == FRAMES_PER_BLOCK.0 as usize {
            // copy entire chan
            let mut block = Block::empty();
            for chan in 0..chans {
                block.push_chan(&buffers[chan as usize][self.playback_offset..next_offset]);
            }
            inputs.blocks.push(block)
        } else {
            // silent fill and copy
            let mut block = Block::default();
            block.repeat(chans);
            block.explicit_repeat();
            for chan in 0..chans {
                let data = block.data_chan_mut(chan);
                let (_, data) = data.split_at_mut(0);
                let (data, _) = data.split_at_mut(samples_to_copy);
                data.copy_from_slice(&buffers[chan as usize][self.playback_offset..next_offset]);
            }
            inputs.blocks.push(block)
        }

        self.playback_offset = next_offset;

        inputs
    }

    fn input_count(&self) -> u32 {
        0
    }

    fn output_count(&self) -> u32 {
        // XXX handle two channels only for now.
        2
    }
}

struct MediaElementSourceNodeRenderer {
    buffers: Arc<Mutex<Vec<Vec<f32>>>>,
    channels: HashMap<u32, usize>,
}

impl MediaElementSourceNodeRenderer {
    pub fn new(buffers: Arc<Mutex<Vec<Vec<f32>>>>) -> Self {
        Self {
            buffers,
            channels: HashMap::new(),
        }
    }
}

impl AudioRenderer for MediaElementSourceNodeRenderer {
    fn render(&mut self, sample: Box<dyn AsRef<[f32]>>, channel_pos: u32) {
        let channel = match self.channels.entry(channel_pos) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let mut buffers = self.buffers.lock().unwrap();
                let len = buffers.len();
                buffers.resize(len + 1, Vec::new());
                *entry.insert(buffers.len())
            }
        };
        self.buffers.lock().unwrap()[(channel - 1) as usize].extend_from_slice((*sample).as_ref());
    }
}
