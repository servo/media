use audio::byte_slice_cast::*;
use audio::node::AudioNodeEngine;
use audio::num_traits::cast::NumCast;

pub struct OscillatorNode {}

impl OscillatorNode {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioNodeEngine for OscillatorNode {
    fn process(
        &self,
        data: &mut [u8],
        accumulator_ref: &mut f64,
        freq: u32,
        rate: u32,
        channels: u32,
        vol: f64,
    ) {
        use std::f64::consts::PI;

        // Reinterpret our byte-slice as a slice containing elements of the type
        // we're interested in. GStreamer requires for raw audio that the alignment
        // of memory is correct, so this will never ever fail unless there is an
        // actual bug elsewhere.
        let data = data.as_mut_slice_of::<f32>().unwrap();

        // Convert all our parameters to the target type for calculations
        let vol: f32 = NumCast::from(vol).unwrap();
        let freq = freq as f64;
        let rate = rate as f64;
        let two_pi = 2.0 * PI;

        // We're carrying a accumulator with up to 2pi around instead of working
        // on the sample offset. High sample offsets cause too much inaccuracy when
        // converted to floating point numbers and then iterated over in 1-steps
        let mut accumulator = *accumulator_ref;
        let step = two_pi * freq / rate;

        for chunk in data.chunks_mut(channels as usize) {
            let value = vol * f32::sin(NumCast::from(accumulator).unwrap());
            for sample in chunk {
                *sample = value;
            }

            accumulator += step;
            if accumulator >= two_pi {
                accumulator -= two_pi;
            }
        }

        *accumulator_ref = accumulator;
    }
}
