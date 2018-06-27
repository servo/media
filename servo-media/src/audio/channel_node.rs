use audio::node::ChannelCountMode;
use audio::block::FRAMES_PER_BLOCK_USIZE;
use audio::node::AudioNodeEngine;
use audio::block::{Block, Chunk};
use audio::node::BlockInfo;

pub struct ChannelNodeOptions {
    pub channels: u8,
}

#[derive(AudioNodeCommon)]
pub struct ChannelMergerNode {
    channels: u8
}

impl ChannelMergerNode {
    pub fn new(params: ChannelNodeOptions) -> Self {
        ChannelMergerNode {
            channels: params.channels
        }
    }
}

impl AudioNodeEngine for ChannelMergerNode {
    fn process(&mut self, mut inputs: Chunk, _: &BlockInfo) -> Chunk {
        debug_assert!(inputs.len() == self.channels as usize);

        let mut block = Block::default();
        block.repeat(self.channels);
        block.explicit_repeat();

        for (i, channel) in block.data_mut().chunks_mut(FRAMES_PER_BLOCK_USIZE).enumerate() {
            channel.copy_from_slice(inputs.blocks[i].data_mut())
        }

        inputs.blocks.clear();
        inputs.blocks.push(block);
        inputs
    }


    fn input_count(&self) -> u32 {
        self.channels as u32
    }

    fn channel_count_mode(&self) -> ChannelCountMode {
        ChannelCountMode::Explicit
    }

}

#[derive(AudioNodeCommon)]
pub struct ChannelSplitterNode {
    channels: u8
}

impl ChannelSplitterNode {
    pub fn new(params: ChannelNodeOptions) -> Self {
        ChannelSplitterNode {
            channels: params.channels
        }
    }
}

impl AudioNodeEngine for ChannelSplitterNode {
    fn process(&mut self, mut inputs: Chunk, _: &BlockInfo) -> Chunk {
        debug_assert!(inputs.len() == 1);

        let original = inputs.blocks.pop().unwrap();

        for chan in 0..original.chan_count() {
            let mut block = Block::empty();
            block.push_chan(original.data_chan(chan));
            inputs.blocks.push(block);
        }

        inputs
    }

    fn output_count(&self) -> u32 {
        self.channels as u32
    }

    fn channel_count_mode(&self) -> ChannelCountMode {
        ChannelCountMode::Explicit
    }

}
