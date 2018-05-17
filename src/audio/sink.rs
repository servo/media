use audio::graph_thread::AudioGraphThread;
use std::sync::Arc;

pub trait AudioSink {
    fn init(&self, Arc<AudioGraphThread>) -> Result<(), ()>;
    fn play(&self);
    fn stop(&self);
}
