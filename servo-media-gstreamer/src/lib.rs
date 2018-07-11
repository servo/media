use servo_media::audio::context::AudioBackend;

extern crate gstreamer_app as gst_app;
extern crate gstreamer_audio as gst_audio;
extern crate gstreamer as gst;

extern crate servo_media;

extern crate num_traits;
extern crate byte_slice_cast;

pub mod audio_decoder;
pub mod audio_sink;

pub struct GStreamerBackend;

impl AudioBackend for GStreamerBackend {
    type Decoder = audio_decoder::GStreamerAudioDecoder;
    type Sink = audio_sink::GStreamerAudioSink;
    fn make_decoder() -> Self::Decoder {
        audio_decoder::GStreamerAudioDecoder::new()
    }
    fn make_sink() -> Result<Self::Sink, ()> {
        audio_sink::GStreamerAudioSink::new()
    }
}