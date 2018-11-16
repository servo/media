use glib;
use glib::prelude::*;
use glib::translate::*;
use glib_ffi;
use gobject_ffi;
use gobject_subclass::object::*;
use gst;
use gst_app::{self, AppSrc, AppSrcCallbacks, AppStreamType};
use gst_ffi;
use gst_plugin::bin::*;
use gst_plugin::element::{ElementClassExt, ElementImpl};
use gst_plugin::object::ElementInstanceStruct;
use gst_plugin::uri_handler::{register_uri_handler, URIHandlerImpl, URIHandlerImplStatic};
use std::ptr;
use std::mem;
use std::sync::{Once, ONCE_INIT};

mod imp {
    use super::*;

    macro_rules! inner_appsrc_proxy {
        ($fn_name:ident, $arg1:ident, $arg1_type:ty, $return_type:ty) => (
            pub fn $fn_name(&self, $arg1: $arg1_type) -> Result<$return_type, ()> {
                match self.appsrc {
                    Some(ref appsrc) => Ok(appsrc.$fn_name($arg1)),
                    None => Err(()),
                }
            }
        )
    }

    pub struct ServoSrc {
        appsrc: Option<gst_app::AppSrc>,
    }

    impl ServoSrc {
        fn init(_bin: &Bin) -> Box<BinImpl<Bin>> {
            let appsrc = gst::ElementFactory::make("appsrc", None)
                .map(|e| e.downcast::<AppSrc>().unwrap());

            Box::new(Self {
                appsrc,
            })
        }

        pub fn get_type() -> glib::Type {
            static ONCE: Once = ONCE_INIT;
            static mut TYPE: glib::Type = glib::Type::Invalid;

            ONCE.call_once(|| {
                let t = register_type(ServoSrcStatic);
                unsafe {
                    TYPE = t;
                }
            });

            unsafe { TYPE }
        }

        fn class_init(klass: &mut BinClass) {
            klass.set_metadata(
                "Servo Media Source",
                "Source/Audio/Video",
                "Feed player with media data",
                "Servo developers",
            );

            let caps = gst::Caps::new_simple(
                "video/x-raw",
                &[
                    ("format", &"BGRA"),
                    ("pixel-aspect-ratio", &gst::Fraction::from((1, 1))),
                ],
            );

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            );

            klass.add_pad_template(src_pad_template);
        }

        pub fn connect_need_data<F: Fn(&AppSrc, u32) + Send + Sync + 'static>(
            &self,
            f: F
        ) -> Result<glib::SignalHandlerId, ()> {
            match self.appsrc {
                Some(ref appsrc) => Ok(appsrc.connect_need_data(f)),
                None => Err(()),
            }
        }
        pub fn end_of_stream(&self) -> Result<gst::FlowReturn, ()> {
            match self.appsrc {
                Some(ref appsrc) => Ok(appsrc.end_of_stream()),
                None => Err(()),
            }
        }

        inner_appsrc_proxy!(push_buffer, buffer, gst::Buffer, gst::FlowReturn);
        inner_appsrc_proxy!(set_callbacks, callbacks, AppSrcCallbacks, ());
        inner_appsrc_proxy!(set_property_format, format, gst::Format, ());
        inner_appsrc_proxy!(set_size, size, i64, ());
        inner_appsrc_proxy!(set_stream_type, type_, AppStreamType, ());
    }

    impl ObjectImpl<Bin> for ServoSrc { }

    impl ElementImpl<Bin> for ServoSrc { }

    impl BinImpl<Bin> for ServoSrc { }

    impl URIHandlerImpl for ServoSrc {
        fn get_uri(&self, _element: &gst::URIHandler) -> Option<String> {
            Some("servosrc://".to_string())
        }

        fn set_uri(&self, _element: &gst::URIHandler, _uri: Option<String>) -> Result<(), glib::Error> {
            Ok(())
        }
    }

    pub struct ServoSrcStatic;

    impl ImplTypeStatic<Bin> for ServoSrcStatic {
        fn get_name(&self) -> &str {
            "ServoSrc"
        }

        fn new(&self, bin: &Bin) -> Box<BinImpl<Bin>> {
            ServoSrc::init(bin)
        }

        fn class_init(&self, klass: &mut BinClass) {
            ServoSrc::class_init(klass);
        }

        fn type_init(&self, token: &TypeInitToken, type_: glib::Type) {
            register_uri_handler(token, type_, self);
        }
    }

    impl URIHandlerImplStatic<Bin> for ServoSrcStatic {
        fn get_impl<'a>(&self, imp: &'a Box<BinImpl<Bin>>) -> &'a URIHandlerImpl {
            imp.downcast_ref::<ServoSrc>().unwrap()
        }

        fn get_type(&self) -> gst::URIType {
            gst::URIType::Src
        }

        fn get_protocols(&self) -> Vec<String> {
            vec!["servosrc".into()]
        }
    }
}

glib_wrapper! {
    pub struct ServoSrc(Object<imp::ServoSrc>):
        [Bin => ElementInstanceStruct<Bin>,
         gst::Bin => gst_ffi::GstBin,
         gst::Element => gst_ffi::GstElement,
         gst::Object => gst_ffi::GstObject];

    match fn {
        get_type => || imp::ServoSrc::get_type().to_glib(),
    }
}

gobject_subclass_deref!(ServoSrc, Bin);

unsafe impl Send for ServoSrc {}
unsafe impl Sync for ServoSrc {}

impl ServoSrc {
    pub fn new() -> ServoSrc {
        use glib::object::Downcast;

        unsafe {
            glib::Object::new(Self::static_type(), &[])
                .unwrap()
                .downcast_unchecked()
        }
    }
}

pub fn register_servo_src() -> bool {
    let type_ = imp::ServoSrc::get_type();
    gst::Element::register(None, "servosrc", 0, type_)
}
