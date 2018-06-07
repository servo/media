use audio::node::{AudioNodeEngine, BlockInfo};
use audio::block::Chunk;

pub struct DestinationNode {}

impl DestinationNode {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioNodeEngine for DestinationNode {
    fn process(&mut self, inputs: Chunk, _: &BlockInfo) -> Chunk {
        inputs
    }
}
