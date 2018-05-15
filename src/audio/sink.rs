use audio::graph::AudioGraph;
use std::sync::Arc;

pub trait AudioSink {
    fn init(&self, Arc<AudioGraph>) -> Result<(), ()>;
    fn play(&self);
    fn stop(&self);
}
