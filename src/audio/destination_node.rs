use audio::node::AudioNodeEngine;
use audio::block::Chunk;

pub struct DestinationNode {}

impl DestinationNode {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioNodeEngine for DestinationNode {
    fn process(&self, inputs: Chunk, _sample_rate: f32) -> Chunk {
        inputs
    }
}
