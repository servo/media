use audio::block::Tick;
use audio::param::Param;
use audio::node::BlockInfo;
use audio::block::Chunk;
use audio::node::{AudioNodeEngine, AudioNodeMessage};
use num_traits::cast::NumCast;

pub struct PeriodicWaveOptions {
    // XXX https://webaudio.github.io/web-audio-api/#dictdef-periodicwaveoptions
}

pub enum OscillatorType {
    Sine,
    Square,
    Sawtooth,
    Triangle,
    Custom,
}

pub struct OscillatorNodeOptions {
    pub oscillator_type: OscillatorType,
    pub freq: f32,
    pub detune: f32,
    pub periodic_wave_options: Option<PeriodicWaveOptions>,
}

impl Default for OscillatorNodeOptions {
    fn default() -> Self {
        OscillatorNodeOptions {
            oscillator_type: OscillatorType::Sine,
            freq: 440.,
            detune: 0.,
            periodic_wave_options: None,
        }
    }
}

pub struct OscillatorNode {
    frequency: Param,
    accumulator: f64,
}

impl OscillatorNode {
    pub fn new(options: OscillatorNodeOptions) -> Self {
        Self {
            frequency: Param::new(options.freq.into()),
            accumulator: 0.,
        }
    }

    pub fn update_parameters(&mut self, info: &BlockInfo, tick: Tick) -> bool {
        self.frequency.update(info, tick)
    }
}

impl AudioNodeEngine for OscillatorNode {
    fn process(
        &mut self,
        mut inputs: Chunk,
        info: &BlockInfo,
    ) -> Chunk {
        // XXX Implement this properly and according to self.options
        // as defined in https://webaudio.github.io/web-audio-api/#oscillatornode

        use std::f64::consts::PI;

        debug_assert!(inputs.len() == 0);

        inputs.blocks.push(Default::default());


        {
            let data = &mut inputs.blocks[0].data;

            // Convert all our parameters to the target type for calculations
            let vol: f32 = 1.0;
            let freq = self.frequency.value() as f64;
            let sample_rate = info.sample_rate as f64;
            let two_pi = 2.0 * PI;

            // We're carrying a accumulator with up to 2pi around instead of working
            // on the sample offset. High sample offsets cause too much inaccuracy when
            // converted to floating point numbers and then iterated over in 1-steps
            let mut step = two_pi * freq / sample_rate;
            let mut tick = Tick(0);
            for sample in data.iter_mut() {
                if self.update_parameters(info, tick) {
                    step = two_pi * freq / sample_rate;
                }
                let value = vol * f32::sin(NumCast::from(self.accumulator).unwrap());
                *sample = value;

                self.accumulator += step;
                if self.accumulator >= two_pi {
                    self.accumulator -= two_pi;
                }
                tick.advance();
            }
        }
        inputs
    }
    fn message(&mut self, msg: AudioNodeMessage, sample_rate: f32) {
        match msg {
            AudioNodeMessage::SetAudioParamEvent(event) => {
                self.frequency.insert_event(event.to_event(sample_rate))
            }
        }
    }
}
