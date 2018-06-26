use audio::node::ChannelCountMode;
use audio::block::Chunk;
use audio::block::Tick;
use audio::node::AudioNodeEngine;
use audio::node::BlockInfo;
use audio::param::{Param, UserAutomationEvent};

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
    fn process(&mut self, mut inputs: Chunk, info: &BlockInfo) -> Chunk {
        debug_assert!(inputs.len() == 1);

        if inputs.blocks[0].is_silence() {
            return inputs
        }

        {
            let mut iter = inputs.blocks[0].iter();
            let mut gain = self.gain.value();

            while let Some(mut frame) = iter.next() {
                if self.update_parameters(info, frame.tick()) {
                    gain = self.gain.value();
                }
                frame.mutate_with(|sample| *sample = *sample * gain);
            }
        }
        inputs
    }

    fn channel_count_mode(&self) -> ChannelCountMode {
        ChannelCountMode::Max
    }

    make_message_handler!(GainNode);
}
