use audio::node::AudioNodeMessage;
use audio::param::Param;
use audio::block::Tick;
use audio::node::BlockInfo;
use audio::node::AudioNodeEngine;
use audio::block::Chunk;

pub struct GainNodeOptions {
    pub gain: f32,
}

impl Default for GainNodeOptions {
    fn default() -> Self {
        GainNodeOptions {
            gain: 1.
        }
    }
}

pub struct GainNode {
    gain: Param,
}

impl GainNode {
    pub fn new(options: GainNodeOptions) -> Self {
        Self {
            gain: Param::new(options.gain)
        }
    }

    pub fn update_parameters(&mut self, info: &BlockInfo, tick: Tick) -> bool {
        self.gain.update(info, tick)
    }
}

impl AudioNodeEngine for GainNode {
    fn process(
        &mut self,
        mut inputs: Chunk,
        info: &BlockInfo,
    ) -> Chunk {
        debug_assert!(inputs.len() == 1);

        {
            let data = &mut inputs.blocks[0].data;

            let mut gain = self.gain.value();
            let mut tick = Tick(0);
            for sample in data.iter_mut() {
                if self.update_parameters(info, tick) {
                    gain = self.gain.value();
                }
                *sample = *sample * gain;
                tick.advance();
            }
        }
        inputs
    }
    fn message(&mut self, msg: AudioNodeMessage, sample_rate: f32) {
        match msg {
            AudioNodeMessage::SetAudioParamEvent(event) => {
                self.gain.insert_event(event.to_event(sample_rate))
            }
        }
    }
}
