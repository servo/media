use block::Chunk;
use block::Tick;
use node::AudioNodeEngine;
use node::BlockInfo;
use node::{AudioNodeType, ChannelInfo};
use param::{Param, ParamType};

#[derive(Copy, Clone, Debug)]
pub struct BiquadFilterNodeOptions {
}

impl Default for BiquadFilterNodeOptions {
    fn default() -> Self {
        BiquadFilterNodeOptions { }
    }
}

#[derive(AudioNodeCommon)]
pub(crate) struct BiquadFilterNode {
    channel_info: ChannelInfo,
}

impl BiquadFilterNode {
    pub fn new(options: BiquadFilterNodeOptions, channel_info: ChannelInfo) -> Self {
        Self {
            channel_info,
        }
    }

    pub fn update_parameters(&mut self, info: &BlockInfo, tick: Tick) -> bool {
        true
    }
}

impl AudioNodeEngine for BiquadFilterNode {
    fn node_type(&self) -> AudioNodeType {
        AudioNodeType::BiquadFilterNode
    }

    fn process(&mut self, mut inputs: Chunk, info: &BlockInfo) -> Chunk {
        inputs
    }

    fn get_param(&mut self, id: ParamType) -> &mut Param {
        match id {
            _ => panic!("Unknown param {:?} for BiquadFilterNode", id),
        }
    }
}
