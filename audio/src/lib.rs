#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate servo_media_derive;

extern crate boxfnonce;
extern crate byte_slice_cast;
extern crate euclid;
extern crate num_traits;
extern crate petgraph;
extern crate smallvec;
#[macro_use]
pub mod macros;

pub mod analyser_node;
pub mod biquad_filter_node;
pub mod block;
pub mod buffer_source_node;
pub mod channel_node;
pub mod constant_source_node;
pub mod context;
pub mod decoder;
pub mod destination_node;
pub mod gain_node;
pub mod graph;
pub mod listener;
pub mod node;
pub mod offline_sink;
pub mod oscillator_node;
pub mod panner_node;
pub mod param;
pub mod render_thread;
pub mod sink;

pub trait AudioBackend {
    type Sink: sink::AudioSink + 'static;
    fn make_decoder() -> Box<decoder::AudioDecoder>;
    fn make_sink() -> Result<Self::Sink, sink::AudioSinkError>;
}
