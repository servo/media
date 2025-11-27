use std::any::Any;

pub enum AppendMode {
    Segments,
    Sequence,
}

pub enum EosError {
    Network,
    Decode,
}

pub enum ReadyState {
    Closed,
    Open,
    Ended,
}

pub trait SourceBuffer: Any + Send {
    fn append_mode(&self) -> AppendMode;
    fn set_append_mode(&self, mode: AppendMode);
    fn updating(&self) -> bool;
    fn abort(&self) -> Result<(), ()>;
    fn timestamp_offset(&self) -> Option<f64>;
    fn set_timestamp_offset(&self, offset: f64);
    fn append_window_start(&self) -> Option<f64>;
    fn set_append_window_start(&self, start: f64);
    fn append_window_end(&self) -> Option<f64>;
    fn set_append_window_end(&self, end: f64);

    fn append_buffer(&self, data: Vec<u8>);
    fn remove(&self, start: f64, end: f64);

    fn on_update_start(&self, f: Box<dyn Fn() + Send + Sync>);
    fn on_update(&self, f: Box<dyn Fn() + Send + Sync>);
    fn on_update_end(&self, f: Box<dyn Fn() + Send + Sync>);
    fn on_error(&self, f: Box<dyn Fn() + Send + Sync>);
    fn on_abort(&self, f: Box<dyn Fn() + Send + Sync>);

    fn as_any(&self) -> &dyn Any;
}

pub trait SourceBufferList: Send {
    fn length(&self) -> u32;
    fn index(&self, index: u32) -> Option<Box<dyn SourceBuffer>>;

    fn on_add_source_buffer(&self, f: Box<dyn Fn() + Send + Sync>);
    fn on_remove_source_buffer(&self, f: Box<dyn Fn() + Send + Sync>);
}

pub trait MediaSource: Send {
    fn add_source_buffer(&self, ty: &str) -> Box<dyn SourceBuffer>;
    fn remove_source_buffer(&self, source_buffer: &dyn SourceBuffer);
    fn live_seekable_range(&self) -> (Option<f64>, Option<f64>);
    fn set_live_seekable_range(&self, start: Option<f64>, end: Option<f64>);
    fn clear_live_seekable_range(&self);
    fn active_source_buffers(&self) -> Box<dyn SourceBufferList>;
    fn source_buffers(&self) -> Box<dyn SourceBufferList>;
    fn duration(&self) -> Option<f64>;
    fn set_duration(&self, duration: f64);
    fn end_of_stream(&self, error: Option<EosError>);
    fn ready_state(&self) -> ReadyState;

    fn on_source_open(&self, f: Box<dyn Fn(&dyn MediaSource) + Send + Sync>);
    fn on_source_ended(&self, f: Box<dyn Fn(&dyn MediaSource) + Send + Sync>);
    fn on_source_close(&self, f: Box<dyn Fn(&dyn MediaSource) + Send + Sync>);

    fn as_any(&self) -> &dyn Any;
}
