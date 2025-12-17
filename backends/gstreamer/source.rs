use glib::subclass::prelude::*;
use gst::prelude::*;
use gst::subclass::prelude::*;
use once_cell::sync::Lazy;
use std::convert::TryFrom;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use url::Url;

const MAX_SRC_QUEUE_SIZE: u64 = 50 * 1024 * 1024; // 50 MB.

// Implementation sub-module of the GObject
mod imp {
    use super::*;

    macro_rules! inner_appsrc_proxy {
        ($fn_name:ident, $return_type:ty) => {
            pub fn $fn_name(&self) -> $return_type {
                self.appsrc.$fn_name()
            }
        };

        ($fn_name:ident, $arg1:ident, $arg1_type:ty, $return_type:ty) => {
            pub fn $fn_name(&self, $arg1: $arg1_type) -> $return_type {
                self.appsrc.$fn_name($arg1)
            }
        };
    }

    #[derive(Debug, Default)]
    struct Position {
        offset: u64,
        requested_offset: u64,
    }

    // The actual data structure that stores our values. This is not accessible
    // directly from the outside.
    pub struct ServoSrc {
        cat: gst::DebugCategory,
        appsrc: gst_app::AppSrc,
        srcpad: gst::GhostPad,
        position: Mutex<Position>,
        seeking: AtomicBool,
        size: Mutex<Option<i64>>,
    }

    impl ServoSrc {
        pub fn set_size(&self, size: i64) {
            if self.seeking.load(Ordering::Relaxed) {
                // We ignore set_size requests if we are seeking.
                // The size value is temporarily stored so it
                // is properly set once we are done seeking.
                *self.size.lock().unwrap() = Some(size);
                return;
            }

            if self.appsrc.size() == -1 {
                self.appsrc.set_size(size);
            }
        }

        pub fn set_seek_offset<O: IsA<gst::Object>>(&self, parent: &O, offset: u64) -> bool {
            let mut pos = self.position.lock().unwrap();

            if pos.offset == offset || pos.requested_offset != 0 {
                false
            } else {
                self.seeking.store(true, Ordering::Relaxed);
                pos.requested_offset = offset;
                gst::debug!(
                    self.cat,
                    obj = parent,
                    "seeking to offset: {}",
                    pos.requested_offset
                );

                true
            }
        }

        pub fn set_seek_done(&self) {
            self.seeking.store(false, Ordering::Relaxed);

            if let Some(size) = self.size.lock().unwrap().take() {
                if self.appsrc.size() == -1 {
                    self.appsrc.set_size(size);
                }
            }

            let mut pos = self.position.lock().unwrap();
            pos.offset = pos.requested_offset;
            pos.requested_offset = 0;
        }

        pub fn push_buffer<O: IsA<gst::Object>>(
            &self,
            parent: &O,
            data: Vec<u8>,
        ) -> Result<gst::FlowSuccess, gst::FlowError> {
            if self.seeking.load(Ordering::Relaxed) {
                gst::debug!(self.cat, obj = parent, "seek in progress, ignored data");
                return Ok(gst::FlowSuccess::Ok);
            }

            let mut pos = self.position.lock().unwrap(); // will block seeking

            let length = u64::try_from(data.len()).unwrap();
            let mut data_offset = 0;

            let buffer_starting_offset = pos.offset;

            // @TODO: optimization: update the element's blocksize by
            // X factor given current length

            pos.offset += length;

            gst::trace!(self.cat, obj = parent, "offset: {}", pos.offset);

            // set the stream size (in bytes) to current offset if
            // size is lesser than it
            let _ = u64::try_from(self.appsrc.size()).and_then(|size| {
                if pos.offset > size {
                    gst::debug!(
                        self.cat,
                        obj = parent,
                        "Updating internal size from {} to {}",
                        size,
                        pos.offset
                    );
                    let new_size = i64::try_from(pos.offset).unwrap();
                    self.appsrc.set_size(new_size);
                }
                Ok(())
            });

            // Split the received vec<> into buffers that are of a
            // size basesrc suggest. It is important not to push
            // buffers that are too large, otherwise incorrect
            // buffering messages can be sent from the pipeline
            let block_size = 4096;
            let num_blocks = ((length - data_offset) as f64 / block_size as f64).ceil() as u64;

            gst::log!(
                self.cat,
                obj = parent,
                "Splitting the received vec into {} blocks",
                num_blocks
            );

            let mut ret: Result<gst::FlowSuccess, gst::FlowError> = Ok(gst::FlowSuccess::Ok);
            for i in 0..num_blocks {
                let start = usize::try_from(i * block_size + data_offset).unwrap();
                data_offset = 0;
                let size = usize::try_from(block_size.min((length - start as u64).into())).unwrap();
                let end = start + size;

                let buffer_offset = buffer_starting_offset + start as u64;
                let buffer_offset_end = buffer_offset + size as u64;

                let subdata = Vec::from(&data[start..end]);
                let mut buffer = gst::Buffer::from_slice(subdata);
                {
                    let buffer = buffer.get_mut().unwrap();
                    buffer.set_offset(buffer_offset);
                    buffer.set_offset_end(buffer_offset_end);
                }

                if self.seeking.load(Ordering::Relaxed) {
                    gst::trace!(
                        self.cat,
                        obj = parent,
                        "stopping buffer appends due to seek"
                    );
                    ret = Ok(gst::FlowSuccess::Ok);
                    break;
                }

                gst::trace!(self.cat, obj = parent, "Pushing buffer {:?}", buffer);

                ret = self.appsrc.push_buffer(buffer);
                match ret {
                    Ok(_) => (),
                    Err(gst::FlowError::Eos) | Err(gst::FlowError::Flushing) => {
                        ret = Ok(gst::FlowSuccess::Ok)
                    },
                    Err(_) => break,
                }
            }

            ret
        }

        inner_appsrc_proxy!(end_of_stream, Result<gst::FlowSuccess, gst::FlowError>);
        inner_appsrc_proxy!(set_callbacks, callbacks, gst_app::AppSrcCallbacks, ());

        fn query(&self, pad: &gst::GhostPad, query: &mut gst::QueryRef) -> bool {
            gst::log!(self.cat, obj = pad, "Handling query {:?}", query);

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
            let ret = match query.view_mut() {
                gst::QueryViewMut::Scheduling(ref mut q) => {
                    let flags =
                        gst::SchedulingFlags::SEQUENTIAL | gst::SchedulingFlags::BANDWIDTH_LIMITED;
                    q.set(flags, 1, -1, 0);
                    q.add_scheduling_modes([gst::PadMode::Push]);
                    true
                },
                _ => gst::Pad::query_default(pad, Some(&*self.obj()), query),
            };

            if ret {
                gst::log!(self.cat, obj = pad, "Handled query {:?}", query);
            } else {
                gst::info!(self.cat, obj = pad, "Didn't handle query {:?}", query);
            }
            ret
        }
    }

    // Basic declaration of our type for the GObject type system
    #[glib::object_subclass]
    impl ObjectSubclass for ServoSrc {
        const NAME: &'static str = "ServoSrc";
        type Type = super::ServoSrc;
        type ParentType = gst::Bin;
        type Interfaces = (gst::URIHandler,);

        // Called once at the very beginning of instantiation of each instance and
        // creates the data structure that contains all our state
        fn with_class(klass: &Self::Class) -> Self {
            let app_src = gst::ElementFactory::make("appsrc")
                .build()
                .map(|elem| elem.downcast::<gst_app::AppSrc>().unwrap())
                .expect("Could not create appsrc element");

            let pad_templ = klass.pad_template("src").unwrap();
            let ghost_pad = gst::GhostPad::builder_from_template(&pad_templ)
                .query_function(|pad, parent, query| {
                    ServoSrc::catch_panic_pad_function(
                        parent,
                        || false,
                        |servosrc| servosrc.query(pad, query),
                    )
                })
                .build();

            Self {
                cat: gst::DebugCategory::new(
                    "servosrc",
                    gst::DebugColorFlags::empty(),
                    Some("Servo source"),
                ),
                appsrc: app_src,
                srcpad: ghost_pad,
                position: Mutex::new(Default::default()),
                seeking: AtomicBool::new(false),
                size: Mutex::new(None),
            }
        }
    }

    // The ObjectImpl trait provides the setters/getters for GObject properties.
    // Here we need to provide the values that are internally stored back to the
    // caller, or store whatever new value the caller is providing.
    //
    // This maps between the GObject properties and our internal storage of the
    // corresponding values of the properties.
    impl ObjectImpl for ServoSrc {
        // Called right after construction of a new instance
        fn constructed(&self) {
            // Call the parent class' ::constructed() implementation first
            self.parent_constructed();

            self.obj()
                .add(&self.appsrc)
                .expect("Could not add appsrc element to bin");

            let target_pad = self.appsrc.static_pad("src");
            self.srcpad.set_target(target_pad.as_ref()).unwrap();

            self.obj()
                .add_pad(&self.srcpad)
                .expect("Could not add source pad to bin");

            self.appsrc.set_caps(None::<&gst::Caps>);
            self.appsrc.set_max_bytes(MAX_SRC_QUEUE_SIZE);
            self.appsrc.set_block(false);
            self.appsrc.set_format(gst::Format::Bytes);
            self.appsrc
                .set_stream_type(gst_app::AppStreamType::Seekable);

            self.obj().set_element_flags(gst::ElementFlags::SOURCE);
        }
    }

    impl GstObjectImpl for ServoSrc {}

    // Implementation of gst::Element virtual methods
    impl ElementImpl for ServoSrc {
        fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
            static ELEMENT_METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
                gst::subclass::ElementMetadata::new(
                    "Servo Media Source",
                    "Source/Audio/Video",
                    "Feed player with media data",
                    "Servo developers",
                )
            });

            Some(&*ELEMENT_METADATA)
        }

        fn pad_templates() -> &'static [gst::PadTemplate] {
            static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
                let caps = gst::Caps::new_any();
                let src_pad_template = gst::PadTemplate::new(
                    "src",
                    gst::PadDirection::Src,
                    gst::PadPresence::Always,
                    &caps,
                )
                .unwrap();

                vec![src_pad_template]
            });

            PAD_TEMPLATES.as_ref()
        }
    }

    // Implementation of gst::Bin virtual methods
    impl BinImpl for ServoSrc {}

    impl URIHandlerImpl for ServoSrc {
        const URI_TYPE: gst::URIType = gst::URIType::Src;

        fn protocols() -> &'static [&'static str] {
            &["servosrc"]
        }

        fn uri(&self) -> Option<String> {
            Some("servosrc://".to_string())
        }

        fn set_uri(&self, uri: &str) -> Result<(), glib::Error> {
            if let Ok(uri) = Url::parse(uri) {
                if uri.scheme() == "servosrc" {
                    return Ok(());
                }
            }
            Err(glib::Error::new(
                gst::URIError::BadUri,
                format!("Invalid URI '{:?}'", uri,).as_str(),
            ))
        }
    }
}

// Public part of the ServoSrc type. This behaves like a normal
// GObject binding
glib::wrapper! {
    pub struct ServoSrc(ObjectSubclass<imp::ServoSrc>)
        @extends gst::Bin, gst::Element, gst::Object, @implements gst::URIHandler;
}

unsafe impl Send for ServoSrc {}
unsafe impl Sync for ServoSrc {}

impl ServoSrc {
    pub fn set_size(&self, size: i64) {
        self.imp().set_size(size);
    }

    pub fn set_seek_offset(&self, offset: u64) -> bool {
        self.imp().set_seek_offset(self, offset)
    }

    pub fn set_seek_done(&self) {
        self.imp().set_seek_done();
    }

    pub fn push_buffer(&self, data: Vec<u8>) -> Result<gst::FlowSuccess, gst::FlowError> {
        self.imp().push_buffer(self, data)
    }

    pub fn push_end_of_stream(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        self.imp().end_of_stream()
    }

    pub fn set_callbacks(&self, callbacks: gst_app::AppSrcCallbacks) {
        self.imp().set_callbacks(callbacks)
    }
}

// Registers the type for our element, and then registers in GStreamer
// under the name "servosrc" for being able to instantiate it via e.g.
// gst::ElementFactory::make().
pub fn register_servo_src() -> Result<(), glib::BoolError> {
    gst::Element::register(None, "servosrc", gst::Rank::NONE, ServoSrc::static_type())
}
