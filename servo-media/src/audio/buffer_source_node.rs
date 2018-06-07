use audio::block::{Chunk, Tick};
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
            let data = inputs.blocks[0].data_mut();

            let mut tick = Tick(0);
            for sample in data.iter_mut() {
                let (should_play_at, should_break) = self.should_play_at(info.frame + tick);
                if !should_play_at {
                    if should_break {
                        break;
                    }
                    continue;
                }

                if self.offset >= self.buffer.len() {
                    break;
                }

                *sample = self.buffer[self.offset];
                self.offset += 1;

                tick.advance();
            }
        }

        inputs
    }

    make_message_handler!(AudioBufferSourceNode);
}
