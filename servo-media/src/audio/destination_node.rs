use audio::node::ChannelInfo;
use audio::node::ChannelCountMode;
use audio::node::{AudioNodeEngine, BlockInfo};
use audio::block::Chunk;

#[derive(AudioNodeCommon)]
pub struct DestinationNode {
    channel_info: ChannelInfo,
    chunk: Option<Chunk>
}

impl DestinationNode {
    pub fn new() -> Self {
        DestinationNode {
            channel_info: ChannelInfo {
                    mode: ChannelCountMode::Explicit,
                    ..Default::default()
            },
            chunk: None,
        }
    }
}

impl AudioNodeEngine for DestinationNode {
    fn process(&mut self, inputs: Chunk, _: &BlockInfo) -> Chunk {
        self.chunk = Some(inputs);
        Chunk::default()
    }

    fn destination_data(&mut self) -> Option<Chunk> {
        self.chunk.take()
    } 

    fn output_count(&self) -> u32 {
        0
    }

    fn set_channel_count_mode(&mut self, _: ChannelCountMode) {
        panic!("destination nodes cannot have their mode changed");
    }
}
