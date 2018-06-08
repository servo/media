use audio::block::{Chunk, Tick, FRAMES_PER_BLOCK};
use audio::node::{AudioNodeEngine, BlockInfo};

pub enum AudioBufferSourceNodeMessage {
    // XXX handle channels
    SetBuffer(Vec<f32>),
    Start(f64),
    Stop(f64),
}

/// AudioBufferSourceNode engine.
#[derive(AudioScheduledSourceNode)]
pub struct AudioBufferSourceNode {
    /// A data block holding the audio sample data.
    buffer: Vec<f32>,
    /// Playback offset.
    offset: usize,
    /// Time at which the source should start playing.
    start_at: Option<Tick>,
    /// Time at which the source should stop playing.
    stop_at: Option<Tick>,
}

impl AudioBufferSourceNode {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            offset: 0,
            start_at: None,
            stop_at: None,
        }
    }

    pub fn handle_message(&mut self, message: AudioBufferSourceNodeMessage, sample_rate: f32) {
        match message {
            AudioBufferSourceNodeMessage::SetBuffer(buffer) => {
                self.buffer = buffer;
            }
            AudioBufferSourceNodeMessage::Start(when) => {
                self.start(Tick::from_time(when, sample_rate));
            }
            AudioBufferSourceNodeMessage::Stop(when) => {
                self.stop(Tick::from_time(when, sample_rate));
            }
        }
    }
}

impl AudioNodeEngine for AudioBufferSourceNode {
    fn input_count(&self) -> u32 { 0 }
    fn process(&mut self, mut inputs: Chunk, info: &BlockInfo) -> Chunk {
        debug_assert!(inputs.len() == 0);

        inputs.blocks.push(Default::default());

        if self.offset >= self.buffer.len() || self.should_play_at(info.frame) == (false, true) {
            return inputs;
        }

        {
            let samples_to_copy = match self.stop_at {
                Some(stop_at) => {
                    let ticks_to_stop = stop_at - info.frame;
                    (if ticks_to_stop > FRAMES_PER_BLOCK {
                        FRAMES_PER_BLOCK
                    } else {
                        ticks_to_stop
                    }).0 as usize
                }
                None => FRAMES_PER_BLOCK.0 as usize,
            };
            let data = inputs.blocks[0].data_mut();
            let (data, _) = data.split_at_mut(samples_to_copy);
            let next_offset = self.offset + samples_to_copy;
            data.copy_from_slice(&self.buffer[self.offset..next_offset]);
            self.offset = next_offset;
        }

        inputs
    }

    make_message_handler!(AudioBufferSourceNode);
}
