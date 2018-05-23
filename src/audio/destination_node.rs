use audio::node::AudioNodeEngine;
use audio::block::Chunk;

pub struct DestinationNode {}

impl DestinationNode {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioNodeEngine for DestinationNode {
    fn process(&self, inputs: Chunk, _rate: u32) -> Chunk {
        inputs
    }
}
