use block::Chunk;
use block::Tick;
use node::AudioNodeEngine;
use node::BlockInfo;
use node::{AudioNodeMessage, AudioNodeType, ChannelInfo};
use param::{Param, ParamType};

#[derive(Copy, Clone, Debug)]
pub struct BiquadFilterNodeOptions {
    filter: FilterType,
    frequency: f32,
    detune: f32,
    q: f32,
    gain: f32,
}

#[derive(Copy, Clone, Debug)]
pub enum FilterType {
    LowPass,
    HighPass,
    BandPass,
    LowShelf,
    HighShelf,
    Peaking,
    Notch,
    Allpass
}

impl Default for BiquadFilterNodeOptions {
    fn default() -> Self {
        BiquadFilterNodeOptions {
            filter: FilterType::LowPass,
            frequency: 350.,
            detune: 0.,
            q: 1.,
            gain: 0.,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum BiquadFilterNodeMessage {
    SetFilterType(FilterType)
}

#[derive(AudioNodeCommon)]
pub(crate) struct BiquadFilterNode {
    channel_info: ChannelInfo,
    filter: FilterType,
    frequency: Param,
    detune: Param,
    q: Param,
    gain: Param,
}

impl BiquadFilterNode {
    pub fn new(options: BiquadFilterNodeOptions, channel_info: ChannelInfo) -> Self {
        Self {
            channel_info,
            filter: options.filter,
            frequency: Param::new(options.frequency),
            gain: Param::new(options.gain),
            q: Param::new(options.q),
            detune: Param::new(options.detune),
        }
    }

    pub fn update_parameters(&mut self, info: &BlockInfo, tick: Tick) -> bool {
        let mut changed = self.frequency.update(info, tick);
        changed |= self.detune.update(info, tick);
        changed |= self.q.update(info, tick);
        changed |= self.gain.update(info, tick);
        changed
    }
}

impl AudioNodeEngine for BiquadFilterNode {
    fn node_type(&self) -> AudioNodeType {
        AudioNodeType::BiquadFilterNode
    }

    fn process(&mut self, mut inputs: Chunk, info: &BlockInfo) -> Chunk {
        // TODO
        inputs
    }

    fn get_param(&mut self, id: ParamType) -> &mut Param {
        match id {
            ParamType::Frequency => &mut self.frequency,
            ParamType::Detune => &mut self.detune,
            ParamType::Q => &mut self.q,
            ParamType::Gain => &mut self.gain,
            _ => panic!("Unknown param {:?} for BiquadFilterNode", id),
        }
    }

    fn message_specific(&mut self, message: AudioNodeMessage, _sample_rate: f32) {
        match message {
            AudioNodeMessage::BiquadFilterNode(m) => {
                match m {
                    BiquadFilterNodeMessage::SetFilterType(f) => self.filter = f,
                }
            }
            _ => ()
        }
    }
}
