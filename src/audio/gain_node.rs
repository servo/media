use audio::block::{Chunk, Tick};
use audio::node::{AudioNodeEngine, BlockInfo};
use audio::param::{Param, UserAutomationEvent};
use std::any::Any;

pub enum GainNodeMessage {
    SetGain(UserAutomationEvent),
}

pub struct GainNodeOptions {
    pub gain: f32,
}

impl Default for GainNodeOptions {
    fn default() -> Self {
        GainNodeOptions { gain: 1. }
    }
}

pub struct GainNode {
    gain: Param,
}

impl GainNode {
    pub fn new(options: GainNodeOptions) -> Self {
        Self {
            gain: Param::new(options.gain),
        }
    }

    pub fn update_parameters(&mut self, info: &BlockInfo, tick: Tick) -> bool {
        self.gain.update(info, tick)
    }

    pub fn handle_message(&mut self, message: GainNodeMessage, sample_rate: f32) {
        match message {
            GainNodeMessage::SetGain(event) => self.gain.insert_event(event.to_event(sample_rate)),
        }
    }
}

impl AudioNodeEngine for GainNode {
    fn process(&mut self, mut inputs: Chunk, info: &BlockInfo) -> Option<Chunk> {
        debug_assert!(inputs.len() == 1);

        {
            let data = &mut inputs.blocks[0].data;

            let mut gain = self.gain.value();
            let mut tick = Tick(0);
            for sample in data.iter_mut() {
                if self.update_parameters(info, tick) {
                    gain = self.gain.value();
                }
                *sample = *sample * gain;
                tick.advance();
            }
        }

        Some(inputs)
    }

    make_message_handler!(GainNode);

    fn as_any(&self) -> &Any {
        self
    }
}
