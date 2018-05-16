use super::gst_app::{AppSrc, AppSrcCallbacks};
use super::gst_audio;
use super::gst_base::prelude::*;
use gst;

// XXX not needed at some point.
use super::byte_slice_cast::*;
use super::num_traits::cast::NumCast;
use super::num_traits::float::Float;

// Default values of properties
const DEFAULT_SAMPLES_PER_BUFFER: u32 = 1024;
const DEFAULT_FREQ: u32 = 440;
const DEFAULT_VOLUME: f64 = 0.8;
const DEFAULT_MUTE: bool = false;
const DEFAULT_IS_LIVE: bool = false;

// Property value storage
#[derive(Debug, Clone, Copy)]
struct Settings {
    samples_per_buffer: u32,
    freq: u32,
    volume: f64,
    mute: bool,
    is_live: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            samples_per_buffer: DEFAULT_SAMPLES_PER_BUFFER,
            freq: DEFAULT_FREQ,
            volume: DEFAULT_VOLUME,
            mute: DEFAULT_MUTE,
            is_live: DEFAULT_IS_LIVE,
        }
    }
}

fn process<F: Float + FromByteSlice>(
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
    let data = data.as_mut_slice_of::<F>().unwrap();

    // Convert all our parameters to the target type for calculations
    let vol: F = NumCast::from(vol).unwrap();
    let freq = freq as f64;
    let rate = rate as f64;
    let two_pi = 2.0 * PI;

    // We're carrying a accumulator with up to 2pi around instead of working
    // on the sample offset. High sample offsets cause too much inaccuracy when
    // converted to floating point numbers and then iterated over in 1-steps
    let mut accumulator = *accumulator_ref;
    let step = two_pi * freq / rate;

    for chunk in data.chunks_mut(channels as usize) {
        let value = vol * F::sin(NumCast::from(accumulator).unwrap());
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

pub fn app_src_oscillator() -> Result<gst::Element, ()> {
    let src = gst::ElementFactory::make("appsrc", None).ok_or(())?;
    let src = src.downcast::<AppSrc>().map_err(|_| ())?;
    let info = gst_audio::AudioInfo::new(gst_audio::AUDIO_FORMAT_F32, 48000, 1)
        .build()
        .ok_or(())?;
    src.set_caps(&info.to_caps().unwrap());
    src.set_property_format(gst::Format::Time);
    let settings = Settings::default();
    let mut sample_offset = 0;
    let mut accumulator = 0.;
    let n_samples = settings.samples_per_buffer as u64;
    let buf_size = (n_samples as usize) * (info.bpf() as usize);
    let rate = info.rate();

    // AudioSrc::process::<f64>(&mut vec, &mut 0., DEFAULT_FREQ, 1 / samples as u32, 1, DEFAULT_VOLUME);
    let need_data = move |app: &AppSrc, _bytes| {
        let mut buffer = gst::Buffer::with_size(buf_size).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();
            // Calculate the current timestamp (PTS) and the next one,
            // and calculate the duration from the difference instead of
            // simply the number of samples to prevent rounding errors
            let pts = sample_offset
                .mul_div_floor(gst::SECOND_VAL, rate as u64)
                .unwrap()
                .into();
            let next_pts: gst::ClockTime = (sample_offset + n_samples)
                .mul_div_floor(gst::SECOND_VAL, rate as u64)
                .unwrap()
                .into();
            buffer.set_pts(pts);
            buffer.set_duration(next_pts - pts);
            let mut map = buffer.map_writable().unwrap();
            let data = map.as_mut_slice();
            process::<f32>(
                data,
                &mut accumulator,
                settings.freq,
                rate,
                1,
                settings.volume,
            );
            sample_offset += n_samples;
        }
        let _ = app.push_buffer(buffer);
    };
    src.set_callbacks(AppSrcCallbacks::new().need_data(need_data).build());
    Ok(src.upcast())
}
