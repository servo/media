use block::Chunk;
use node::{AudioNodeEngine, AudioNodeType, BlockInfo, ChannelInfo};

#[derive(Clone, Debug, PartialEq)]
pub enum OverSampleType {
    None,
    Double,
    Quadruple,
}

type WaveShaperCurve = Option<Vec<f32>>;

#[derive(Clone, Debug)]
pub struct WaveShaperNodeOptions {
    pub curve: WaveShaperCurve,
    pub oversample: OverSampleType,
}

impl Default for WaveShaperNodeOptions {
    fn default() -> Self {
        WaveShaperNodeOptions {
            curve: None,
            oversample: OverSampleType::None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum WaveShaperNodeMessage {
    SetCurve(WaveShaperCurve),
}

#[derive(AudioNodeCommon)]
pub(crate) struct WaveShaperNode {
    curve_set: bool,
    curve: WaveShaperCurve,
    #[allow(dead_code)]
    // TODO implement tail-time based on the oversample attribute.
    // https://github.com/servo/media/issues/205
    oversample: OverSampleType,
    channel_info: ChannelInfo,
}

impl WaveShaperNode {
    pub fn new(options: WaveShaperNodeOptions, channel_info: ChannelInfo) -> Self {
        if let Some(vec) = &options.curve {
            assert!(
                vec.len() > 1,
                "WaveShaperNode curve must have length of 2 or more"
            )
        }
        if options.oversample != OverSampleType::None {
            unimplemented!("No oversampling for WaveShaperNode yet");
        }

        Self {
            curve_set: options.curve.is_some(),
            curve: options.curve,
            oversample: options.oversample,
            channel_info,
        }
    }

    fn handle_waveshaper_message(&mut self, message: WaveShaperNodeMessage, _sample_rate: f32) {
        match message {
            WaveShaperNodeMessage::SetCurve(new_curve) => {
                if self.curve_set && new_curve.is_some() {
                    panic!("InvalidStateError: cant set curve if it was already set");
                }
                self.curve_set = new_curve.is_some();
                self.curve = new_curve;
            }
        }
    }
}

impl AudioNodeEngine for WaveShaperNode {
    fn node_type(&self) -> AudioNodeType {
        AudioNodeType::WaveShaperNode
    }

    fn process(&mut self, mut inputs: Chunk, _info: &BlockInfo) -> Chunk {
        debug_assert!(inputs.len() == 1);

        if inputs.blocks[0].is_silence() {
            return inputs;
        }

        if let Some(curve) = &self.curve {
            let mut iter = inputs.blocks[0].iter();

            while let Some(mut frame) = iter.next() {
                frame.mutate_with(|sample, _| {
                    let len = curve.len();
                    let curve_index: f32 = ((len - 1) as f32) * (*sample + 1.) / 2.;

                    if curve_index <= 0. {
                        *sample = curve[0];
                    } else if curve_index >= len as f32 {
                        *sample = curve[len - 1];
                    } else {
                        let index_lo = curve_index as usize;
                        let index_hi = index_lo + 1;
                        let interp_factor: f32 = curve_index - index_lo as f32;
                        *sample = (1. - interp_factor) * curve[index_lo]
                            + interp_factor * curve[index_hi];
                    }
                });
            }

            inputs
        } else {
            inputs
        }
    }

    make_message_handler!(WaveShaperNode: handle_waveshaper_message);
}
