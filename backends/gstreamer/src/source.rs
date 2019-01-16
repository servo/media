use glib;
use glib::translate::*;
use glib_ffi;
use gobject_ffi;
use gobject_subclass::object::*;
use gst;
use gst::query::{QueryRef, QueryView};
use gst_app::{self, AppSrc, AppSrcCallbacks, AppStreamType};
use gst_ffi;
use gst_plugin::bin::*;
use gst_plugin::element::ElementImpl;
use gst_plugin::object::ElementInstanceStruct;
use gst_plugin::uri_handler::{register_uri_handler, URIHandlerImpl, URIHandlerImplStatic};
use std::mem;
use std::ptr;
use std::sync::{Once, ONCE_INIT};
use url::Url;

const MAX_SRC_QUEUE_SIZE: u64 = 50 * 1024 * 1024; // 50 MB.

mod imp {
    use super::*;
    use glib::prelude::*;
    use gst::ElementExt;
    use gst_plugin::element::ElementClassExt;

    macro_rules! inner_appsrc_proxy {
        ($fn_name:ident, $return_type:ty) => (
            pub fn $fn_name(&self) -> $return_type {
                self.appsrc.$fn_name()
            }
        );

        ($fn_name:ident, $arg1:ident, $arg1_type:ty, $return_type:ty) => (
            pub fn $fn_name(&self, $arg1: $arg1_type) -> $return_type {
                self.appsrc.$fn_name($arg1)
            }
        )
    }

    pub struct ServoSrc {
        appsrc: gst_app::AppSrc,
    }

    impl ServoSrc {
        fn init(_bin: &Bin) -> Box<BinImpl<Bin>> {
            let appsrc = gst::ElementFactory::make("appsrc", None)
                .map(|elem| elem.downcast::<AppSrc>().unwrap())
                .expect("Could not create appsrc element");

            appsrc.set_max_bytes(MAX_SRC_QUEUE_SIZE);
            appsrc.set_property_block(false);
            appsrc.set_property_format(gst::Format::Bytes);

            // At this point the bin is not completely created,
            // so we cannot add anything to it yet.
            // We have to wait until ObjectImpl::constructed.
            Box::new(Self { appsrc })
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
        }

        pub fn set_size(&self, size: i64) {
            if self.appsrc.get_size() != -1 {
                return;
            }
            self.appsrc.set_size(size);
        }

        inner_appsrc_proxy!(end_of_stream, gst::FlowReturn);
        inner_appsrc_proxy!(get_current_level_bytes, u64);
        inner_appsrc_proxy!(get_max_bytes, u64);
        inner_appsrc_proxy!(push_buffer, buffer, gst::Buffer, gst::FlowReturn);
        inner_appsrc_proxy!(set_callbacks, callbacks, AppSrcCallbacks, ());
        inner_appsrc_proxy!(set_stream_type, type_, AppStreamType, ());
    }

    impl ObjectImpl<Bin> for ServoSrc {
        fn constructed(&self, bin: &Bin) {
            bin.parent_constructed();

            self.add_element(bin, &self.appsrc.clone().upcast());

            let pad = self
                .appsrc
                .get_static_pad("src")
                .expect("Could not get src pad");

            let ghost_pad =
                gst::GhostPad::new("src", &pad).expect("Could not create src ghost pad");

            bin.add_pad(&ghost_pad)
                .expect("Could not add src ghost pad to bin");
        }
    }

    impl ElementImpl<Bin> for ServoSrc {
        fn query(&self, _element: &Bin, query: &mut QueryRef) -> bool {
            // In order to make buffering/downloading work as we want, apart from
            // setting the appropriate flags on the player playbin,
            // the source needs to either:
            //
            // 1. be an http, mms, etc. scheme
            // 2. report that it is "bandwidth limited".
            //
            // 1. is not straightforward because we are using a servosrc scheme for now.
            // This may change in the future if we end up handling http/https/data
            // URIs, which is what WebKit does.
            //
            // For 2. we need to make servosrc handle the scheduling properties query
            // to report that it "is bandwidth limited".
             if let QueryView::Scheduling(ref mut query) = query.view_mut() {
                query.set(
                    gst::SchedulingFlags::SEQUENTIAL
                        | gst::SchedulingFlags::BANDWIDTH_LIMITED,
                    1,
                    -1,
                    0,
                );
                query.add_scheduling_modes(&[gst::PadMode::Push]);
                return true;
            }
            false
        }
    }

    impl BinImpl<Bin> for ServoSrc {}

    impl URIHandlerImpl for ServoSrc {
        fn get_uri(&self, _element: &gst::URIHandler) -> Option<String> {
            Some("servosrc://".to_string())
        }

        fn set_uri(
            &self,
            _element: &gst::URIHandler,
            uri: Option<String>,
        ) -> Result<(), glib::Error> {
            if let Some(ref uri) = uri {
                if let Ok(uri) = Url::parse(uri) {
                    if uri.scheme() == "servosrc" {
                        return Ok(())
                    }
                }
            }
            Err(glib::Error::new(
                gst::URIError::BadUri,
                format!("Invalid URI '{:?}'", uri,).as_str(),
            ))
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

impl ServoSrc {}

pub fn register_servo_src() -> bool {
    let type_ = imp::ServoSrc::get_type();
    gst::Element::register(None, "servosrc", 0, type_)
}
