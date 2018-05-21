use std::cell::Cell;
use audio::byte_slice_cast::*;
use audio::node::AudioNodeEngine;
use audio::num_traits::cast::NumCast;

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
    accumulator: Cell<f64>
}

impl OscillatorNode {
    pub fn new(options: OscillatorNodeOptions) -> Self {
        Self { options, accumulator: Cell::new(0.) }
    }
}

impl AudioNodeEngine for OscillatorNode {
    fn process(
        &self,
        data: &mut [u8],
        rate: u32,
    ) {
        // XXX Implement this properly and according to self.options
        // as defined in https://webaudio.github.io/web-audio-api/#oscillatornode

        use std::f64::consts::PI;

        // Reinterpret our byte-slice as a slice containing elements of the type
        // we're interested in. GStreamer requires for raw audio that the alignment
        // of memory is correct, so this will never ever fail unless there is an
        // actual bug elsewhere.
        let data = data.as_mut_slice_of::<f32>().unwrap();

        // Convert all our parameters to the target type for calculations
        let vol: f32 = 1.0;
        let freq = self.options.freq as f64;
        let rate = rate as f64;
        let two_pi = 2.0 * PI;

        // We're carrying a accumulator with up to 2pi around instead of working
        // on the sample offset. High sample offsets cause too much inaccuracy when
        // converted to floating point numbers and then iterated over in 1-steps
        let step = two_pi * freq / rate;
        let mut accumulator = self.accumulator.get();

        for sample in data {
            let value = vol * f32::sin(NumCast::from(accumulator).unwrap());
            *sample = value;

            accumulator += step;
            if accumulator >= two_pi {
                accumulator -= two_pi;
            }
        }
        self.accumulator.set(accumulator);
    }
}
