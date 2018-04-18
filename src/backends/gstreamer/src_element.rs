use gst;
use std::i32;
use std::ops::Rem;
use std::sync::Mutex;
use super::gst_audio;
use super::gst_base::prelude::*;
use super::gst_plugin::base_src::*;
use super::gst_plugin::element::*;
use super::gst_plugin::object::*;

// XXX not needed at some point.
use super::num_traits::float::Float;
use super::num_traits::cast::NumCast;
use super::byte_slice_cast::*;

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

// Stream-specific state, i.e. audio format configuration
// and sample offset
struct State {
    info: Option<gst_audio::AudioInfo>,
    sample_offset: u64,
    sample_stop: Option<u64>,
    accumulator: f64,
}

impl Default for State {
    fn default() -> State {
        State {
            info: None,
            sample_offset: 0,
            sample_stop: None,
            accumulator: 0.0,
        }
    }
}

// Struct containing all the element data
struct AudioSrc {
    cat: gst::DebugCategory,
    state: Mutex<State>,
    settings: Mutex<Settings>,
}

impl AudioSrc {
    // Called when a new instance is to be created
    fn new(element: &BaseSrc) -> Box<BaseSrcImpl<BaseSrc>> {
        // Initialize live-ness and notify the base class that
        // we'd like to operate in Time format
        element.set_live(false);
        element.set_format(gst::Format::Time);

        Box::new(Self {
            cat: gst::DebugCategory::new(
                "servoaudiosrc",
                gst::DebugColorFlags::empty(),
                "Servo Audio Source",
            ),
            state: Mutex::new(Default::default()),
            settings: Mutex::new(Default::default()),
        })
    }

    // Called exactly once when registering the type. Used for
    // setting up metadata for all instances, e.g. the name and
    // classification and the pad templates with their caps.
    //
    // Actual instances can create pads based on those pad templates
    // with a subset of the caps given here. In case of basesrc,
    // only a "src" pad template is required here and the base class
    // will automatically instantiate a pad for it.
    //
    fn class_init(klass: &mut BaseSrcClass) {
        klass.set_metadata(
            "Servo Audio Source",
            "Source/Audio",
            "Creates sound",
            "Fernando Jimenez Moreno <ferjmoreno@gmail.com>",
        );

        // On the src pad, we can produce F32 with any sample rate
        // and any number of channels
        let caps = gst::Caps::new_simple(
            "audio/x-raw",
            &[
                (
                    "format",
                    &gst::List::new(&[&gst_audio::AUDIO_FORMAT_F32.to_string()]),
                ),
                ("layout", &"interleaved"),
                ("rate", &gst::IntRange::<i32>::new(1, i32::MAX)),
                ("channels", &gst::IntRange::<i32>::new(1, i32::MAX)),
            ],
        );
        // The src pad template must be named "src" for basesrc
        // and specific a pad that is always there
        let src_pad_template = gst::PadTemplate::new(
            "src",
            gst::PadDirection::Src,
            gst::PadPresence::Always,
            &caps,
        );
        klass.add_pad_template(src_pad_template);
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
}

impl ObjectImpl<BaseSrc> for AudioSrc {}

// Virtual methods of gst::Element. We override none
impl ElementImpl<BaseSrc> for AudioSrc {}

impl BaseSrcImpl<BaseSrc> for AudioSrc {
    // Called when starting, so we can initialize all stream-related state to its defaults
    fn start(&self, element: &BaseSrc) -> bool {
        // Reset state
        *self.state.lock().unwrap() = Default::default();

        gst_info!(self.cat, obj: element, "Started");

        true
    }

    // Called when shutting down the element so we can release all stream-related state
    fn stop(&self, element: &BaseSrc) -> bool {
        // Reset state
        *self.state.lock().unwrap() = Default::default();

        gst_info!(self.cat, obj: element, "Stopped");

        true
    }

    fn set_caps(&self, element: &BaseSrc, caps: &gst::CapsRef) -> bool {
        use std::f64::consts::PI;

        let info = match gst_audio::AudioInfo::from_caps(caps) {
            None => return false,
            Some(info) => info,
        };

        gst_debug!(self.cat, obj: element, "Configuring for caps {}", caps);

        element.set_blocksize(info.bpf() * (*self.settings.lock().unwrap()).samples_per_buffer);

        let settings = *self.settings.lock().unwrap();
        let mut state = self.state.lock().unwrap();

        // If we have no caps yet, any old sample_offset and sample_stop will be
        // in nanoseconds
        let old_rate = match state.info {
            Some(ref info) => info.rate() as u64,
            None => gst::SECOND_VAL,
        };

        // Update sample offset and accumulator based on the previous values and the
        // sample rate change, if any
        let old_sample_offset = state.sample_offset;
        let sample_offset = old_sample_offset
            .mul_div_floor(info.rate() as u64, old_rate)
            .unwrap();

        let old_sample_stop = state.sample_stop;
        let sample_stop =
            old_sample_stop.map(|v| v.mul_div_floor(info.rate() as u64, old_rate).unwrap());

        let accumulator =
            (sample_offset as f64).rem(2.0 * PI * (settings.freq as f64) / (info.rate() as f64));

        *state = State {
            info: Some(info),
            sample_offset: sample_offset,
            sample_stop: sample_stop,
            accumulator: accumulator,
        };

        drop(state);

        let _ = element.post_message(&gst::Message::new_latency().src(Some(element)).build());

        true
    }

    fn create(
        &self,
        element: &BaseSrc,
        _offset: u64,
        _length: u32,
    ) -> Result<gst::Buffer, gst::FlowReturn> {
        // Keep a local copy of the values of all our properties at this very moment. This
        // ensures that the mutex is never locked for long and the application wouldn't
        // have to block until this function returns when getting/setting property values
        let settings = *self.settings.lock().unwrap();

        // Get a locked reference to our state, i.e. the input and output AudioInfo
        let mut state = self.state.lock().unwrap();
        let info = match state.info {
            None => {
                gst_element_error!(element, gst::CoreError::Negotiation, ["Have no caps yet"]);
                return Err(gst::FlowReturn::NotNegotiated);
            }
            Some(ref info) => info.clone(),
        };

        // If a stop position is set (from a seek), only produce samples up to that
        // point but at most samples_per_buffer samples per buffer
        let n_samples = if let Some(sample_stop) = state.sample_stop {
            if sample_stop <= state.sample_offset {
                gst_log!(self.cat, obj: element, "At EOS");
                return Err(gst::FlowReturn::Eos);
            }

            sample_stop - state.sample_offset
        } else {
            settings.samples_per_buffer as u64
        };

        // Allocate a new buffer of the required size, update the metadata with the
        // current timestamp and duration and then fill it according to the current
        // caps
        let mut buffer =
            gst::Buffer::with_size((n_samples as usize) * (info.bpf() as usize)).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();

            // Calculate the current timestamp (PTS) and the next one,
            // and calculate the duration from the difference instead of
            // simply the number of samples to prevent rounding errors
            let pts = state
                .sample_offset
                .mul_div_floor(gst::SECOND_VAL, info.rate() as u64)
                .unwrap()
                .into();
            let next_pts: gst::ClockTime = (state.sample_offset + n_samples)
                .mul_div_floor(gst::SECOND_VAL, info.rate() as u64)
                .unwrap()
                .into();
            buffer.set_pts(pts);
            buffer.set_duration(next_pts - pts);

            // Map the buffer writable and create the actual samples
            let mut map = buffer.map_writable().unwrap();
            let data = map.as_mut_slice();

            if info.format() == gst_audio::AUDIO_FORMAT_F32 {
                Self::process::<f32>(
                    data,
                    &mut state.accumulator,
                    settings.freq,
                    info.rate(),
                    info.channels(),
                    settings.volume,
                );
            } else {
                Self::process::<f64>(
                    data,
                    &mut state.accumulator,
                    settings.freq,
                    info.rate(),
                    info.channels(),
                    settings.volume,
                );
            }
        }
        state.sample_offset += n_samples;
        drop(state);

        gst_debug!(self.cat, obj: element, "Produced buffer {:?}", buffer);

        Ok(buffer)
    }
}

struct AudioSrcStatic;

// The basic trait for registering the type: This returns a name for the type and registers the
// instance and class initialization functions with the type system, thus hooking everything
// together.
impl ImplTypeStatic<BaseSrc> for AudioSrcStatic {
    fn get_name(&self) -> &str {
        "AudioSrc"
    }

    fn new(&self, element: &BaseSrc) -> Box<BaseSrcImpl<BaseSrc>> {
        AudioSrc::new(element)
    }

    fn class_init(&self, klass: &mut BaseSrcClass) {
        AudioSrc::class_init(klass);
    }
}

// Registers the type for our element, and then registers in GStreamer under
// the name "servoaudiosrc" for being able to instantiate it via e.g.
// gst::ElementFactory::make().
pub fn register() {
    let type_ = register_type(AudioSrcStatic);
    gst::Element::register(None, "servoaudiosrc", 0, type_);
}
