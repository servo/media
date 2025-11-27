use gst::{Buffer, ClockTime};
use gst_mse::{SourceBuffer as GstSourceBuffer, SourceBufferAppendMode};
use servo_media_mse::{AppendMode, SourceBuffer};
use std::any::Any;

pub struct GStreamerSourceBuffer {
    inner: GstSourceBuffer,
}

impl GStreamerSourceBuffer {
    pub fn inner(&self) -> &GstSourceBuffer {
        &self.inner
    }
}

impl From<GstSourceBuffer> for GStreamerSourceBuffer {
    fn from(inner: GstSourceBuffer) -> Self {
        GStreamerSourceBuffer { inner }
    }
}

impl SourceBuffer for GStreamerSourceBuffer {
    fn append_mode(&self) -> AppendMode {
        match self.inner.append_mode() {
            SourceBufferAppendMode::Segments => AppendMode::Segments,
            SourceBufferAppendMode::Sequence => AppendMode::Sequence,
            _ => unreachable!(),
        }
    }

    fn set_append_mode(&self, mode: AppendMode) {
        let gst_mode = match mode {
            AppendMode::Segments => SourceBufferAppendMode::Segments,
            AppendMode::Sequence => SourceBufferAppendMode::Sequence,
        };
        self.inner.set_append_mode(gst_mode);
    }

    fn updating(&self) -> bool {
        self.inner.is_updating()
    }

    fn abort(&self) -> Result<(), ()> {
        self.inner.abort().map_err(|_| ())
    }

    fn timestamp_offset(&self) -> Option<f64> {
        self.inner.timestamp_offset().map(|t| t.seconds_f64())
    }

    fn set_timestamp_offset(&self, offset: f64) {
        self.inner
            .set_timestamp_offset(ClockTime::from_seconds_f64(offset));
    }

    fn append_window_start(&self) -> Option<f64> {
        self.inner.append_window_start().map(|t| t.seconds_f64())
    }

    fn set_append_window_start(&self, start: f64) {
        self.inner
            .set_append_window_start(ClockTime::from_seconds_f64(start));
    }

    fn append_window_end(&self) -> Option<f64> {
        self.inner.append_window_end().map(|t| t.seconds_f64())
    }

    fn set_append_window_end(&self, end: f64) {
        self.inner
            .set_append_window_end(ClockTime::from_seconds_f64(end));
    }

    fn append_buffer(&self, data: Vec<u8>) {
        let buffer = Buffer::from_slice(data);
        self.inner.append_buffer(buffer);
    }

    fn remove(&self, start: f64, end: f64) {
        self.inner.remove(
            ClockTime::from_seconds_f64(start),
            ClockTime::from_seconds_f64(end),
        );
    }

    fn on_update_start(&self, f: Box<dyn Fn() + Send + Sync>) {
        self.inner.connect_on_update_start(move |_b| {
            f();
        });
    }

    fn on_update(&self, f: Box<dyn Fn() + Send + Sync>) {
        self.inner.connect_on_update(move |_b| {
            f();
        });
    }

    fn on_update_end(&self, f: Box<dyn Fn() + Send + Sync>) {
        self.inner.connect_on_update_end(move |_b| {
            f();
        });
    }

    fn on_error(&self, f: Box<dyn Fn() + Send + Sync>) {
        self.inner.connect_on_error(move |_b| {
            f();
        });
    }

    fn on_abort(&self, f: Box<dyn Fn() + Send + Sync>) {
        self.inner.connect_on_abort(move |_b| {
            f();
        });
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
