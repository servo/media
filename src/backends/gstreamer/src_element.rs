use gst;
use super::gst_audio;
use super::gst_base::prelude::*;
use super::gst_plugin::base_src::*;
use super::gst_plugin::element::*;
use super::gst_plugin::object::*;

use std::i32;
use std::sync::Mutex;

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
            "Creates a sound",
            "Fernando Jimenez Moreno <ferjmoreno@gmail.com>",
            );

        // On the src pad, we can produce F32 with any sample rate
        // and any number of channels
        let caps = gst::Caps::new_simple(
            "audio/x-raw",
            &[
            (
                "format",
                &gst::List::new(&[
                                &gst_audio::AUDIO_FORMAT_F32.to_string(),
                ]),
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
}

impl ObjectImpl<BaseSrc> for AudioSrc { }

// Virtual methods of gst::Element. We override none
impl ElementImpl<BaseSrc> for AudioSrc { }

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
pub fn register(plugin: &gst::Plugin) {
    let type_ = register_type(AudioSrcStatic);
    gst::Element::register(plugin, "servoaudiosrc", 0, type_);
}

