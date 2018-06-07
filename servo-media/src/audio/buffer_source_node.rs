use audio::block::Chunk;
use audio::node::{AudioNodeEngine, BlockInfo};

pub enum AudioBufferSourceNodeMessage {
    // XXX handle channels
    SetBuffer(Vec<f32>),
}

/// AudioBufferSourceNode engine.
pub struct AudioBufferSourceNode {
    buffer: Vec<f32>,
    offset: usize,
}

impl AudioBufferSourceNode {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            offset: 0,
        }
    }

    pub fn handle_message(&mut self, message: AudioBufferSourceNodeMessage, _sample_rate: f32) {
        match message {
            AudioBufferSourceNodeMessage::SetBuffer(buffer) => {
                self.buffer = buffer;
            }
        }
    }
}

impl AudioNodeEngine for AudioBufferSourceNode {
    fn process(&mut self, mut inputs: Chunk, _info: &BlockInfo) -> Chunk {
        debug_assert!(inputs.len() == 0);

        inputs.blocks.push(Default::default());

        if self.offset >= self.buffer.len() {
            return inputs;
        }

        {
            let data = inputs.blocks[0].data_mut();

            for sample in data.iter_mut() {
                if self.offset >= self.buffer.len() {
                    break;
                }

                *sample = self.buffer[self.offset];
                self.offset += 1;
            }
        }

        inputs
    }

    make_message_handler!(AudioBufferSourceNode);
}
