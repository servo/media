use audio::node::AudioNodeEngine;
use audio::block::Chunk;

pub struct GainNodeOptions {
    pub gain: f32,
}

impl Default for GainNodeOptions {
    fn default() -> Self {
        GainNodeOptions {
            gain: 1.
        }
    }
}

pub struct GainNode {
    options: GainNodeOptions,
}

impl GainNode {
    pub fn new(options: GainNodeOptions) -> Self {
        Self { options }
    }
}

impl AudioNodeEngine for GainNode {
    fn process(
        &self,
        mut inputs: Chunk,
        _rate: u32,
    ) -> Chunk {
        debug_assert!(inputs.len() == 1);

        {
            let data = &mut inputs.blocks[0].data;

            for sample in data.iter_mut() {
                *sample = *sample * self.options.gain
            }
        }
        inputs
    }
}
