use block::{Block, Chunk};
use node::AudioNodeEngine;
use node::BlockInfo;
use node::{AudioNodeType, ChannelInfo, ChannelInterpretation};
use std::sync::mpsc::Sender;


#[derive(AudioNodeCommon)]
pub(crate) struct AnalyserNode {
    channel_info: ChannelInfo,
    sender: Sender<Block>
}

impl AnalyserNode {
    pub fn new(sender: Sender<Block>, channel_info: ChannelInfo) -> Self {
        Self { sender, channel_info }
    }

}

impl AudioNodeEngine for AnalyserNode {
    fn node_type(&self) -> AudioNodeType {
        AudioNodeType::AnalyserNode
    }

    fn process(&mut self, inputs: Chunk, _: &BlockInfo) -> Chunk {
        debug_assert!(inputs.len() == 1);

        let mut push = inputs.blocks[0].clone();
        push.mix(1, ChannelInterpretation::Speakers);

        let _ = self.sender.send(push);

        // analyser node doesn't modify the inputs
        inputs
    }
}
