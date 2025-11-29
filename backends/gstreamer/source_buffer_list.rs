use crate::source_buffer::GStreamerSourceBuffer;
use gst_mse::{SourceBuffer as GstSourceBuffer, SourceBufferList as GstSourceBufferList};
use servo_media_mse::{SourceBuffer, SourceBufferList};

pub struct GStreamerSourceBufferList {
    inner: GstSourceBufferList,
}

impl From<GstSourceBufferList> for GStreamerSourceBufferList {
    fn from(inner: GstSourceBufferList) -> Self {
        GStreamerSourceBufferList { inner }
    }
}

impl SourceBufferList for GStreamerSourceBufferList {
    fn length(&self) -> u32 {
        self.inner.length()
    }

    fn index(&self, index: u32) -> Option<Box<dyn SourceBuffer>> {
        self.inner.index(index).map(|buffer| {
            let source_buffer = GStreamerSourceBuffer::from(buffer);
            Box::new(source_buffer) as Box<dyn SourceBuffer>
        })
    }

    fn on_add_source_buffer(&self, f: Box<dyn Fn() + Send + Sync>) {
        self.inner.connect_on_sourcebuffer_added(move |_l| {
            f();
        });
    }

    fn on_remove_source_buffer(&self, f: Box<dyn Fn() + Send + Sync>) {
        self.inner.connect_on_sourcebuffer_removed(move |_l| {
            f();
        });
    }
}
