use block::Chunk;
use block::Tick;
use node::AudioNodeEngine;
use node::BlockInfo;
use node::{AudioNodeType, ChannelInfo};
use param::{Param, ParamType};

#[derive(Copy, Clone, Debug)]
pub struct ConstantSourceNodeOptions {
    pub offset: f32,
}

impl Default for ConstantSourceNodeOptions {
    fn default() -> Self {
        ConstantSourceNodeOptions { offset: 1. }
    }
}

#[derive(AudioNodeCommon)]
pub(crate) struct ConstantSourceNode {
    channel_info: ChannelInfo,
    offset: Param,
}

impl ConstantSourceNode {
    pub fn new(options: ConstantSourceNodeOptions, channel_info: ChannelInfo) -> Self {
        Self {
            channel_info,
            offset: Param::new(options.offset),
        }
    }

    pub fn update_parameters(&mut self, info: &BlockInfo, tick: Tick) -> bool {
        self.offset.update(info, tick)
    }
}

impl AudioNodeEngine for ConstantSourceNode {
    fn node_type(&self) -> AudioNodeType {
        AudioNodeType::ConstantSourceNode
    }

    fn process(&mut self, mut inputs: Chunk, info: &BlockInfo) -> Chunk {
        debug_assert!(inputs.len() == 1);
        inputs.blocks[0].explicit_silence();
      if inputs.blocks[0].is_silence() {
            return inputs;
        }

        {
            let mut iter = inputs.blocks[0].iter();
            let mut offset = self.offset.value();
            while let Some(mut frame) = iter.next() {
                if self.update_parameters(info, frame.tick()) {
                    offset = self.offset.value();
                }
                
                frame.mutate_with(|sample, _| *sample = offset);
            }
        }
        inputs
    }

    fn get_param(&mut self, id: ParamType) -> &mut Param {
        match id {
            ParamType::Offset => &mut self.offset,
            _ => panic!("Unknown param {:?} for the offset", id),
        }
    }
}
