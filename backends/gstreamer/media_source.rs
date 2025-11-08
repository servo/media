use crate::source_buffer::GStreamerSourceBuffer;
use crate::source_buffer_list::GStreamerSourceBufferList;
use gst::ClockTime;
use gst_mse::{MediaSource as GstMediaSource, MediaSourceEOSError};
use servo_media_mse::{EosError, MediaSource, ReadyState, SourceBuffer, SourceBufferList};

#[repr(transparent)]
pub struct GStreamerMediaSource {
    inner: GstMediaSource,
}

impl GStreamerMediaSource {
    pub fn new() -> Self {
        GStreamerMediaSource {
            inner: GstMediaSource::new(),
        }
    }

    pub(crate) fn inner(&self) -> &GstMediaSource {
        &self.inner
    }

    pub(crate) fn from_ref(inner: &GstMediaSource) -> &Self {
        unsafe { &*(inner as *const GstMediaSource as *const GStreamerMediaSource) }
    }
}

impl MediaSource for GStreamerMediaSource {
    fn add_source_buffer(&self, ty: &str) -> Box<dyn SourceBuffer> {
        let buffer = self.inner.add_source_buffer(ty).unwrap();
        let buffer = GStreamerSourceBuffer::from(buffer);
        Box::new(buffer)
    }

    fn remove_source_buffer(&self, source_buffer: &dyn SourceBuffer) {
        let gst_source_buffer = source_buffer
            .as_any()
            .downcast_ref::<GStreamerSourceBuffer>()
            .expect("Expected a GStreamer SourceBuffer");
        self.inner.remove_source_buffer(gst_source_buffer.inner());
    }

    fn live_seekable_range(&self) -> (Option<f64>, Option<f64>) {
        let range = self.inner.live_seekable_range();
        (
            Some(range.start().seconds_f64()),
            Some(range.end().seconds_f64()),
        )
    }

    fn set_live_seekable_range(&self, start: Option<f64>, end: Option<f64>) {
        let start = start.map(ClockTime::from_seconds_f64);
        let end = end.map(ClockTime::from_seconds_f64);
        self.inner.set_live_seekable_range(start, end);
    }

    fn clear_live_seekable_range(&self) {
        self.inner.clear_live_seekable_range();
    }

    fn active_source_buffers(&self) -> Box<dyn SourceBufferList> {
        let list: GStreamerSourceBufferList = self.inner.active_source_buffers().into();
        Box::new(list)
    }

    fn source_buffers(&self) -> Box<dyn SourceBufferList> {
        let list: GStreamerSourceBufferList = self.inner.source_buffers().into();
        Box::new(list)
    }

    fn duration(&self) -> Option<f64> {
        self.inner.duration().map(|d| d.seconds_f64())
    }

    fn set_duration(&self, duration: f64) {
        self.inner
            .set_duration(ClockTime::from_seconds_f64(duration));
    }

    fn end_of_stream(&self, error: Option<EosError>) {
        let _ = match error {
            Some(EosError::Network) => self.inner.end_of_stream(MediaSourceEOSError::Network),
            Some(EosError::Decode) => self.inner.end_of_stream(MediaSourceEOSError::Decode),
            None => self.inner.end_of_stream(MediaSourceEOSError::None),
        };
    }

    fn ready_state(&self) -> ReadyState {
        match self.inner.ready_state() {
            gst_mse::MediaSourceReadyState::Closed => ReadyState::Closed,
            gst_mse::MediaSourceReadyState::Open => ReadyState::Open,
            gst_mse::MediaSourceReadyState::Ended => ReadyState::Ended,
            _ => unreachable!(),
        }
    }

    fn on_source_open(&self, f: Box<dyn Fn(&dyn MediaSource) + Send + Sync>) {
        self.inner.connect_on_source_open(move |s| {
            f(Self::from_ref(s));
        });
    }

    fn on_source_ended(&self, f: Box<dyn Fn(&dyn MediaSource) + Send + Sync>) {
        self.inner.connect_on_source_ended(move |s| {
            f(Self::from_ref(s));
        });
    }

    fn on_source_close(&self, f: Box<dyn Fn(&dyn MediaSource) + Send + Sync>) {
        self.inner.connect_on_source_close(move |s| {
            f(Self::from_ref(s));
        });
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
