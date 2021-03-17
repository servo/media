pub trait SinkType: Send + Sync + 'static {
    fn get_sink_name() -> String;
}

pub struct AutoSinkType;
impl SinkType for AutoSinkType {
    fn get_sink_name() -> String {
        "autoaudiosink".to_owned()
    }
}

pub struct DummySinkType;
impl SinkType for DummySinkType {
    fn get_sink_name() -> String {
        "fakesink".to_owned()
    }
}
