use glib;
use glib::subclass;
use glib::subclass::prelude::*;
use glib::translate::*;
use gst;
use gst::prelude::*;
use gst::subclass::prelude::*;
use media_stream::GStreamerMediaStream;
use url::Url;

// Implementation sub-module of the GObject
mod imp {
    use super::*;

    pub struct ServoMediaStreamSrc {
        cat: gst::DebugCategory,
        proxysrc: gst::Element,
        srcpad: gst::GhostPad,
    }

    impl ServoMediaStreamSrc {
        pub fn set_stream(&self, stream: &mut GStreamerMediaStream) {
            gst_log!(self.cat, "Setting stream");

            // Append a proxysink to the media stream pipeline.
            let pipeline = stream.pipeline_or_new();
            let last_element = stream.src_element();
            let sink = gst::ElementFactory::make("proxysink", None).unwrap();
            pipeline.add(&sink).unwrap();
            gst::Element::link_many(&[&last_element, &sink][..]).unwrap();

            // Connect the media stream proxysink to the source's proxysrc.
            self.proxysrc.set_property("proxysink", &sink).unwrap();

            sink.sync_state_with_parent().unwrap();

            pipeline.set_state(gst::State::Playing).unwrap();
        }
    }

    // Basic declaration of our type for the GObject type system.
    impl ObjectSubclass for ServoMediaStreamSrc {
        const NAME: &'static str = "ServoMediaStreamSrc";
        type ParentType = gst::Bin;
        type Instance = gst::subclass::ElementInstanceStruct<Self>;
        type Class = subclass::simple::ClassStruct<Self>;

        glib_object_subclass!();

        // Called once at the very beginning of instantiation of each instance and
        // creates the data structure that contains all our state
        fn new_with_class(klass: &subclass::simple::ClassStruct<Self>) -> Self {
            let proxysrc = gst::ElementFactory::make("proxysrc", None)
                .expect("Could not create proxysrc element");

            let pad_templ = klass.get_pad_template("src").unwrap();
            let srcpad =
                gst::GhostPad::new_no_target_from_template("stream_src", &pad_templ).unwrap();

            Self {
                cat: gst::DebugCategory::new(
                    "servomediastreamsrc",
                    gst::DebugColorFlags::empty(),
                    "Servo media stream source",
                ),
                proxysrc,
                srcpad,
            }
        }

        // Adds interface implementations in the class
        fn type_init(type_: &mut subclass::InitializingType<Self>) {
            type_.add_interface::<gst::URIHandler>();
        }

        // Called exactly once before the first instantiation of an instance. This
        // sets up any type-specific things, in this specific case it installs the
        // properties so that GObject knows about their existence and they can be
        // used on instances of our type
        fn class_init(klass: &mut subclass::simple::ClassStruct<Self>) {
            klass.set_metadata(
                "Servo Media Stream Source",
                "Source/Audio/Video",
                "Feed player with media stream data",
                "Servo developers",
            );

            let caps = gst::Caps::new_any();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();
            klass.add_pad_template(src_pad_template);
        }
    }

    // The ObjectImpl trait provides the setters/getters for GObject properties.
    // Here we need to provide the values that are internally stored back to the
    // caller, or store whatever new value the caller is providing.
    //
    // This maps between the GObject properties and our internal storage of the
    // corresponding values of the properties.
    impl ObjectImpl for ServoMediaStreamSrc {
        glib_object_impl!();

        // Called right after construction of a new instance
        fn constructed(&self, obj: &glib::Object) {
            // Call the parent class' ::constructed() implementation first
            self.parent_constructed(obj);

            let bin = obj.downcast_ref::<gst::Bin>().unwrap();

            // Add audio proxy sink and source pad to bin.
            bin.add(&self.proxysrc)
                .expect("Could not add proxysrc element to bin");

            let target_pad = self
                .proxysrc
                .get_static_pad("src")
                .expect("Could not get source pad");
            self.srcpad.set_target(&target_pad).unwrap();

            let element = obj.downcast_ref::<gst::Element>().unwrap();
            element
                .add_pad(&self.srcpad)
                .expect("Could not add source pad to bin");

            ::set_element_flags(element, gst::ElementFlags::SOURCE);
        }
    }

    // Implementation of gst::Element virtual methods
    impl ElementImpl for ServoMediaStreamSrc {}

    // Implementation of gst::Bin virtual methods
    impl BinImpl for ServoMediaStreamSrc {}

    impl URIHandlerImpl for ServoMediaStreamSrc {
        fn get_uri(&self, _element: &gst::URIHandler) -> Option<String> {
            Some("mediastream://".to_string())
        }

        fn set_uri(
            &self,
            _element: &gst::URIHandler,
            uri: Option<String>,
        ) -> Result<(), glib::Error> {
            if let Some(ref uri) = uri {
                if let Ok(uri) = Url::parse(uri) {
                    if uri.scheme() == "mediastream" {
                        return Ok(());
                    }
                }
            }
            Err(glib::Error::new(
                gst::URIError::BadUri,
                format!("Invalid URI '{:?}'", uri,).as_str(),
            ))
        }

        fn get_uri_type() -> gst::URIType {
            gst::URIType::Src
        }

        fn get_protocols() -> Vec<String> {
            vec!["mediastream".into()]
        }
    }
}

// Public part of the ServoMediaStreamSrc type. This behaves like a normal
// GObject binding
glib_wrapper! {
    pub struct ServoMediaStreamSrc(Object<gst::subclass::ElementInstanceStruct<imp::ServoMediaStreamSrc>,
                                   subclass::simple::ClassStruct<imp::ServoMediaStreamSrc>, ServoMediaStreamSrcClass>)
        @extends gst::Bin, gst::Element, gst::Object, @implements gst::URIHandler;

    match fn {
        get_type => || imp::ServoMediaStreamSrc::get_type().to_glib(),
    }
}

unsafe impl Send for ServoMediaStreamSrc {}
unsafe impl Sync for ServoMediaStreamSrc {}

macro_rules! inner_proxy {
    ($fn_name:ident, $arg1:ident, $arg1_type:ty) => (
        pub fn $fn_name(&self, $arg1: $arg1_type) {
            imp::ServoMediaStreamSrc::from_instance(self).$fn_name($arg1)
        }
    )
}

impl ServoMediaStreamSrc {
    inner_proxy!(set_stream, stream, &mut GStreamerMediaStream);
}

// Registers the type for our element, and then registers in GStreamer
// under the name "servomediastreamsrc" for being able to instantiate it via e.g.
// gst::ElementFactory::make().
pub fn register_servo_media_stream_src() -> Result<(), glib::BoolError> {
    gst::Element::register(
        None,
        "servomediastreamsrc",
        0,
        ServoMediaStreamSrc::static_type(),
    )
}
