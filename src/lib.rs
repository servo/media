use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::sync::mpsc::{self, Sender};
use std::sync::{self, Once};
use std::sync::{Arc, Mutex};
use std::thread::Builder;

#[cfg(feature = "gst")]
extern crate gstreamer as gst;

extern crate smallvec;
extern crate byte_slice_cast;
extern crate num_traits;

pub mod audio;
mod backends;
mod media_thread;

pub use audio::graph::AudioGraph;
use media_thread::MediaThreadMsg;

pub struct ServoMedia {
    sender: Sender<MediaThreadMsg>,
}

static INITIALIZER: Once = sync::ONCE_INIT;
static mut INSTANCE: *mut Mutex<Option<Arc<ServoMedia>>> = 0 as *mut _;
static NEXT_GRAPH_ID: AtomicUsize = ATOMIC_USIZE_INIT;

impl ServoMedia {
    pub fn new() -> Self {
        #[cfg(feature = "gst")]
        gst::init().unwrap();

        let (sender, receiver) = mpsc::channel();
        Builder::new()
            .name("ServoMedia".to_owned())
            .spawn(move || {
                media_thread::event_loop(receiver);
            })
            .unwrap();
        Self { sender }
    }

    pub fn get() -> Result<Arc<ServoMedia>, ()> {
        INITIALIZER.call_once(|| unsafe {
            INSTANCE = Box::into_raw(Box::new(Mutex::new(Some(Arc::new(ServoMedia::new())))));
        });
        let instance = unsafe { &*INSTANCE }.lock().unwrap();
        match *instance {
            Some(ref instance) => Ok(instance.clone()),
            None => Err(()),
        }
    }

    pub fn create_audio_graph(&self) -> Result<AudioGraph, ()> {
        let graph_id = NEXT_GRAPH_ID.fetch_add(1, Ordering::SeqCst);
        Ok(AudioGraph::new(graph_id, self.sender.clone()))
    }
}
