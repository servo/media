use audio::node::ChannelCountMode;
use audio::node::{AudioNodeEngine, BlockInfo};
use audio::block::Chunk;

pub struct DestinationNode(Option<Chunk>);

impl DestinationNode {
    pub fn new() -> Self {
        DestinationNode(None)
    }
}

impl AudioNodeEngine for DestinationNode {
    fn process(&mut self, inputs: Chunk, _: &BlockInfo) -> Chunk {
        self.0 = Some(inputs);
        Chunk::default()
    }

    fn destination_data(&mut self) -> Option<Chunk> {
        self.0.take()
    } 

    fn output_count(&self) -> u32 {
        0
    }

    fn channel_count(&self) -> u8 {
        // currently hardcoded here and in our invocation of
        // gst_audio::AudioInfo::new
        2
    }

    fn channel_count_mode(&self) -> ChannelCountMode {
        ChannelCountMode::Explicit
    }
}
