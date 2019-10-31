use block::Chunk;
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
    renderer: Arc<Mutex<dyn AudioRenderer>>,
}

impl MediaElementSourceNode {
    pub fn new(options: MediaElementSourceNodeOptions, channel_info: ChannelInfo) -> Self {
        let renderer = Arc::new(Mutex::new(MediaElementSourceNodeRenderer::new()));
        let _ = options
            .player
            .lock()
            .unwrap()
            .set_audio_renderer(renderer.clone());
        Self {
            channel_info,
            renderer,
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

        inputs.blocks[0].explicit_silence();

        // XXX get data from the renderer's buffer

        inputs
    }

    fn input_count(&self) -> u32 {
        0
    }
}

struct MediaElementSourceNodeRenderer {
    buffer: Vec<Vec<f32>>,
    channels: HashMap<u32, usize>,
}

impl MediaElementSourceNodeRenderer {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            channels: HashMap::new(),
        }
    }
}

impl AudioRenderer for MediaElementSourceNodeRenderer {
    fn render(&mut self, sample: Box<dyn AsRef<[f32]>>, channel_pos: u32) {
        let channel = match self.channels.entry(channel_pos) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                self.buffer.resize(self.buffer.len() + 1, Vec::new());
                *entry.insert(self.buffer.len())
            }
        };
        self.buffer[(channel - 1) as usize].extend_from_slice((*sample).as_ref());
    }
}
