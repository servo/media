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
    options: OscillatorNodeOptions,
    accumulator: f64,
}

impl OscillatorNode {
    pub fn new(options: OscillatorNodeOptions) -> Self {
        Self {
            options,
            accumulator: 0.,
        }
    }
}

impl AudioNodeEngine for OscillatorNode {
    fn process(
        &mut self,
        mut inputs: Chunk,
        sample_rate: f32,
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
            let freq = self.options.freq as f64;
            let sample_rate = sample_rate as f64;
            let two_pi = 2.0 * PI;

            // We're carrying a accumulator with up to 2pi around instead of working
            // on the sample offset. High sample offsets cause too much inaccuracy when
            // converted to floating point numbers and then iterated over in 1-steps
            let step = two_pi * freq / sample_rate;
            let mut accumulator = self.accumulator;

            for sample in data.iter_mut() {
                let value = vol * f32::sin(NumCast::from(accumulator).unwrap());
                *sample = value;

                accumulator += step;
                if accumulator >= two_pi {
                    accumulator -= two_pi;
                }
            }
            self.accumulator = accumulator;
        }
        inputs
    }
    fn message(&mut self, msg: AudioNodeMessage) {
        match msg {
            AudioNodeMessage::SetFloatParam(val) => {
                self.options.freq = val
            }
        }

    }
}
